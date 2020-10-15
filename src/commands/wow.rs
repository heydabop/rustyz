use crate::{WowAuth, WowConfig};
use image::{imageops, png::PngEncoder, ColorType, ImageFormat};
use reqwest::{StatusCode, Url};
use serde::Deserialize;
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandError, CommandResult};
use serenity::http::AttachmentType;
use serenity::model::channel::Message;
use std::borrow::Cow;
use std::time::{Duration, SystemTime};

#[derive(Deserialize)]
struct AuthResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct KeyValue {
    key: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct CharacterMedia {
    assets: Option<Vec<KeyValue>>,
    render_url: Option<String>,
}

async fn auth(client_id: &str, client_secret: &str) -> Result<WowAuth, reqwest::Error> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://us.battle.net/oauth/token")
        .basic_auth(client_id, Some(client_secret))
        .form(&[("grant_type", "client_credentials")])
        .send()
        .await?
        .json::<AuthResponse>()
        .await?;

    let expires_at = SystemTime::now() + Duration::from_secs((resp.expires_in - 60).max(0));

    Ok(WowAuth {
        access_token: resp.access_token,
        expires_at,
    })
}

#[command]
#[aliases("drip")]
pub async fn character(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let arg = args.single::<String>()?;
    let char_realm: Vec<&str> = arg.split('-').collect();
    if char_realm.len() != 2 {
        msg.channel_id
            .say(&ctx.http, "`Usage: /wow chracter name-realm`")
            .await?;
        return Ok(());
    }
    let character = char_realm[0];
    let realm = char_realm[1];

    let access_token = {
        let mut wow_config = {
            let data = ctx.data.read().await;
            data.get::<WowConfig>().unwrap().clone()
        };
        if wow_config.auth.is_none()
            || wow_config.auth.as_ref().unwrap().expires_at < SystemTime::now()
        {
            let new_auth = auth(&wow_config.client_id, &wow_config.client_secret).await?;
            let access_token = new_auth.access_token.clone();
            let mut data = ctx.data.write().await;
            wow_config.auth = Some(new_auth);
            data.insert::<WowConfig>(wow_config);
            access_token
        } else {
            wow_config.auth.unwrap().access_token
        }
    };

    let client = reqwest::Client::new();
    let resp: CharacterMedia = match client.get(Url::parse(&format!("https://us.api.blizzard.com/profile/wow/character/{}/{}/character-media?namespace=profile-us&locale=en_US&access_token={}", realm, character, access_token)).unwrap())
        .send().await {
            Ok(resp) => if resp.status() == StatusCode::NOT_FOUND {
                msg.channel_id.say(&ctx.http, format!("Unable to find {} on {}", character, realm)).await?;
                return Ok(());
            } else {
                resp.json::<CharacterMedia>().await?
            },
            Err(e) => return Err(CommandError::from(e))
        };

    let mut found_raw = false;
    let mut image_url = None;
    if let Some(assets) = resp.assets {
        if let Some(raw) = assets.iter().find(|&a| a.key == "main-raw") {
            found_raw = true;
            image_url = Some(raw.value.clone());
        } else if let Some(main) = assets.iter().find(|&a| a.key == "main") {
            image_url = Some(main.value.clone());
        }
    }
    if image_url.is_none() {
        image_url = resp.render_url;
    }
    if image_url.is_none() {
        return Err(CommandError::from("Unable to find character imagery"));
    }
    if !found_raw {
        msg.channel_id.say(&ctx.http, image_url.unwrap()).await?;
        return Ok(());
    }

    let image_bytes = reqwest::get(Url::parse(&image_url.unwrap()).unwrap())
        .await?
        .bytes()
        .await?;
    let mut image =
        image::load_from_memory_with_format(&image_bytes, ImageFormat::Png)?.into_rgba();
    let (width, height) = image.dimensions();

    let mut top = 0;
    let mut bottom = height;
    let mut left = 0;
    let mut right = width;

    // find top most non-blank pixel
    'top: for y in 0..height {
        for x in 0..width {
            if image[(x, y)][3] > 0 {
                // check alpha channel for non-transparency
                top = y;
                break 'top;
            }
        }
    }
    // find bottom most non-blank pixel
    'bottom: for y in (top..height).rev() {
        for x in 0..width {
            if image[(x, y)][3] > 0 {
                bottom = (y + 1).min(height);
                break 'bottom;
            }
        }
    }
    // find left most non-blank pixel
    'left: for x in 0..width {
        for y in top..bottom {
            if image[(x, y)][3] > 0 {
                left = x;
                break 'left;
            }
        }
    }
    // find right most non-blank pixel
    'right: for x in (left..width).rev() {
        for y in top..bottom {
            if image[(x, y)][3] > 0 {
                right = (x + 1).min(width);
                break 'right;
            }
        }
    }

    let cropped_image =
        imageops::crop(&mut image, left, top, right - left, bottom - top).to_image();
    let mut cropped_buffer = Vec::new();
    PngEncoder::new(&mut cropped_buffer).encode(
        &cropped_image,
        right - left,
        bottom - top,
        ColorType::Rgba8,
    )?;

    msg.channel_id
        .send_message(&ctx.http, |m| {
            m.add_file(AttachmentType::Bytes {
                data: Cow::from(cropped_buffer),
                filename: format!("{}.png", arg),
            });
            m
        })
        .await?;

    Ok(())
}
