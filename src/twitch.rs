use crate::config;
use reqwest::{Client, Error};
use serde::Deserialize;
use serenity::client::Context;
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug, Deserialize)]
pub struct Stream {
    pub user_name: String,
    pub game_name: String,
    pub title: String,
    r#type: String,
    pub viewer_count: u64,
}

#[derive(Deserialize)]
pub struct StreamsResponse {
    data: Vec<Stream>,
}

#[derive(Deserialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub expires_in: u64,
}

pub async fn get_access_token(ctx: &Context) -> Result<(String, String), Error> {
    let mut config = {
        let data = ctx.data.read().await;
        data.get::<config::Twitch>().unwrap().clone()
    };
    let client_id = config.client_id.clone();
    if config.auth.is_none() || config.auth.as_ref().unwrap().expires_at < SystemTime::now() {
        let client = Client::new();
        let resp = client
            .post("https://id.twitch.tv/oauth2/token")
            .form(&[
                ("client_id", &config.client_id),
                ("client_secret", &config.client_secret),
                ("grant_type", &"client_credentials".to_owned()),
            ])
            .send()
            .await?;
        let auth = match resp.error_for_status() {
            Ok(resp) => resp.json::<AuthResponse>().await?,
            Err(e) => return Err(e),
        };
        let mut data = ctx.data.write().await;
        config.auth = Some(config::TwitchAuth {
            access_token: auth.access_token.clone(),
            expires_at: SystemTime::now()
                + Duration::from_secs(if auth.expires_in > 60 {
                    auth.expires_in - 60
                } else {
                    auth.expires_in
                }),
        });
        data.insert::<config::Twitch>(config);
        Ok((auth.access_token, client_id))
    } else {
        Ok((config.auth.unwrap().access_token, client_id))
    }
}

pub async fn get_stream_info(
    auth_token: &str,
    client_id: &str,
    channel_name: &str,
) -> Result<Option<Stream>, Error> {
    let client = Client::new();
    let resp = client
        .get("https://api.twitch.tv/helix/streams")
        .query(&[("first", "1"), ("user_login", channel_name)])
        .bearer_auth(auth_token)
        .header("Client-Id", client_id)
        .send()
        .await?;
    match resp.error_for_status() {
        Ok(resp) => {
            let json = resp.json::<StreamsResponse>().await?;
            if json.data.len() == 1 && json.data[0].r#type == "live" {
                Ok(Some(json.data[0].clone()))
            } else {
                Ok(None)
            }
        }
        Err(e) => Err(e),
    }
}
