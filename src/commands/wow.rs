use crate::{WowAuth, WowConfig};
use chrono::{DateTime, Local, TimeZone};
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

#[derive(Debug, Deserialize)]
struct Character {
    last_login_timestamp: i64,
}

// Get access token from global state or Blizzard API if token missing/expired
async fn get_access_token(ctx: &Context) -> Result<String, reqwest::Error> {
    let mut wow_config = {
        let data = ctx.data.read().await;
        data.get::<WowConfig>().unwrap().clone()
    };
    if wow_config.auth.is_none() || wow_config.auth.as_ref().unwrap().expires_at < SystemTime::now()
    {
        // Fetch new access token if we currently have no auth info or auth has expired
        let new_auth = auth(&wow_config.client_id, &wow_config.client_secret).await?;
        let access_token = new_auth.access_token.clone();
        let mut data = ctx.data.write().await;
        wow_config.auth = Some(new_auth);
        data.insert::<WowConfig>(wow_config);
        Ok(access_token)
    } else {
        // Otherwise return existing saved token
        Ok(wow_config.auth.unwrap().access_token)
    }
}

// Get bearer auth token from Blizzard API with client_id and client_secret
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

    // Mark auth as expiring a little early so that we refresh before absolutely necessary
    let expires_at = SystemTime::now() + Duration::from_secs((resp.expires_in - 60).max(0));

    Ok(WowAuth {
        access_token: resp.access_token,
        expires_at,
    })
}

// Takes in the arg `<character>-<realm>` and replies with an image from WoW's armory
// Tries to get transparency png image and crop it, otherwise returns "deafult" jpg image with background
#[command]
#[aliases("drip")]
pub async fn mog(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // Parse out character and realm names from single string arg `<character>-<realm>`
    let mut arg = args.rest().to_string();
    arg.make_ascii_lowercase();
    let char_realm: Vec<&str> = arg.splitn(2, '-').collect();
    if char_realm.len() != 2 {
        msg.channel_id
            .say(&ctx.http, "`Usage: /wow [drip|mog] name-realm`")
            .await?;
        return Ok(());
    }
    let character = char_realm[0].trim();
    let realm = char_realm[1].trim().replace(" ", "-").replace("'", "");

    let access_token = get_access_token(ctx).await?;

    let date_format = "%a, %b %-d %Y at %-I:%M%P";
    let client = reqwest::Client::new();

    // Get character last login time (and check if they exist)
    let resp = client.get(Url::parse(&format!("https://us.api.blizzard.com/profile/wow/character/{}/{}?namespace=profile-us&locale=en_US&access_token={}", realm, character, access_token)).unwrap())
        .send().await;
    let last_login: String = match resp {
        Ok(resp) => {
            if resp.status() == StatusCode::NOT_FOUND {
                msg.channel_id
                    .say(
                        &ctx.http,
                        format!("Unable to find {} on {}", character, realm),
                    )
                    .await?;
                return Ok(());
            } else {
                let c = resp.json::<Character>().await?;
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                let last_login_date = Local.timestamp(
                    c.last_login_timestamp / 1000,
                    ((c.last_login_timestamp % 1000) * 1000) as u32,
                );
                format!("Player last seen on {}", last_login_date.format(date_format))
            }
        }
        Err(e) => return Err(CommandError::from(e)),
    };

    // Get JSON info of character's appearance
    let resp  = client.get(Url::parse(&format!("https://us.api.blizzard.com/profile/wow/character/{}/{}/character-media?namespace=profile-us&locale=en_US&access_token={}", realm, character, access_token)).unwrap())
        .send().await?;
    let last_modified: Option<String> = match &resp.headers().get("last-modified") {
        Some(lm) => match lm.to_str() {
            Ok(val) => {
                if let Ok(last_modified) = DateTime::parse_from_rfc2822(&val) {
                    Some(format!(
                        "Image updated on {}",
                        last_modified.with_timezone(&Local).format(date_format)
                    ))
                } else {
                    println!("Unable to parse last-modified: {}", val);
                    None
                }
            }
            Err(_) => None,
        },
        None => None,
    };
    let media = resp.json::<CharacterMedia>().await?;

    let msg_content = if let Some(last_modified) = last_modified {
        format!("{}\n{}", last_modified, last_login)
    } else {
        last_login
    };


    let mut found_raw = false; //if we found "row" png image with transparency
    let mut image_url: Option<String> = None; //URL of image we'll use

    // First check assets array for main-raw (PNG) image, then main image
    if let Some(assets) = media.assets {
        if let Some(raw) = assets.iter().find(|&a| a.key == "main-raw") {
            found_raw = true;
            image_url = Some(raw.value.clone());
        } else if let Some(main) = assets.iter().find(|&a| a.key == "main") {
            image_url = Some(main.value.clone());
        }
    }
    // If image wasn't in assets (or it didn't exist) check other render_url field
    if image_url.is_none() {
        image_url = media.render_url;
    }
    if image_url.is_none() {
        return Err(CommandError::from("Unable to find character imagery"));
    }
    // If we didn't find a transparent-background PNG image, just send the URL for whatever image we do have (discord will convert it)
    if !found_raw {
        msg.channel_id
            .say(&ctx.http, msg_content)
            .await?;
        msg.channel_id.say(&ctx.http, image_url.unwrap()).await?;
        return Ok(());
    }

    // Otherwise, fetch, decode, crop, and attach PNG image

    // Fetch and decode image, assume it's a PNG
    let image_bytes = reqwest::get(Url::parse(&image_url.unwrap()).unwrap())
        .await?
        .bytes()
        .await?;
    let mut image =
        image::load_from_memory_with_format(&image_bytes, ImageFormat::Png)?.into_rgba();
    let (width, height) = image.dimensions();

    // Find boundaries of image content
    let mut top = 0;
    let mut bottom = height;
    let mut left = 0;
    let mut right = width;

    // find top most non-blank pixel
    (0..height).any(|y| {
        (0..width).any(|x| {
            if image[(x, y)][3] > 0 {
                // check alpha channel for non-transparency
                top = y;
                return true;
            }
            false
        })
    });

    // find bottom most non-blank pixel
    (top..height).rev().any(|y| {
        (0..width).any(|x| {
            if image[(x, y)][3] > 0 {
                bottom = (y + 1).min(height);
                return true;
            }
            false
        })
    });
    // find left most non-blank pixel
    (0..width).any(|x| {
        (top..bottom).any(|y| {
            if image[(x, y)][3] > 0 {
                left = x;
                return true;
            }
            false
        })
    });
    // find right most non-blank pixel
    (left..width).rev().any(|x| {
        (top..bottom).any(|y| {
            if image[(x, y)][3] > 0 {
                right = (x + 1).min(width);
                return true;
            }
            false
        })
    });

    // Crop image and encode PNG into buffer
    let cropped_image =
        imageops::crop(&mut image, left, top, right - left, bottom - top).to_image();
    let mut cropped_buffer = Vec::new();
    PngEncoder::new(&mut cropped_buffer).encode(
        &cropped_image,
        right - left,
        bottom - top,
        ColorType::Rgba8,
    )?;

    // Send message with attached cropped image
    msg.channel_id
        .send_message(&ctx.http, |m| {
            m.add_file(AttachmentType::Bytes {
                data: Cow::from(cropped_buffer),
                filename: format!("{}.png", arg),
            });
            m.content(msg_content);
            m
        })
        .await?;

    Ok(())
}