use crate::{WowAuth, WowConfig};
use reqwest::{StatusCode, Url};
use serde::Deserialize;
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandError, CommandResult};
use serenity::model::channel::Message;
use std::time::{Duration, SystemTime};

#[derive(Deserialize)]
struct AuthResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Deserialize)]
struct KeyValue {
    key: String,
    value: String,
}

#[derive(Deserialize)]
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

    let mut image_url = String::from("");
    if let Some(url) = resp.render_url {
        image_url = url;
    } else {
        for entry in resp.assets.unwrap() {
            if entry.key == "main" {
                image_url = entry.value;
                break;
            }
        }
    }

    msg.channel_id.say(&ctx.http, image_url).await?;

    Ok(())
}
