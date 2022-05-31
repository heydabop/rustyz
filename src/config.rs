use serde::Deserialize;
use serenity::prelude::*;
use std::time::SystemTime;

#[derive(Deserialize)]
pub struct Discord {
    pub application_id: u64,
    pub bot_token: String,
    pub user_id: u64,
}

#[derive(Deserialize)]
pub struct Google {
    pub maps_api_key: String,
}

#[derive(Deserialize)]
pub struct Psql {
    pub old_url: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct TarkovMarket {
    pub api_key: String,
}

#[derive(Clone, Deserialize)]
pub struct TomorrowIO {
    pub api_key: String,
    pub default_location_id: String,
    pub default_location_name: String,
}

#[derive(Clone, Deserialize)]
pub struct Twitch {
    pub client_id: String,
    pub client_secret: String,
    pub auth: Option<TwitchAuth>, // not populated by config.toml, populated by first request to twitch API
}

#[derive(Clone, Deserialize)]
pub struct TwitchAuth {
    pub access_token: String,
    pub expires_at: SystemTime,
}

#[derive(Clone, Deserialize)]
pub struct Wow {
    pub client_id: String,
    pub client_secret: String,
    pub auth: Option<WowAuth>, // not populated by config.toml, populated by first request to wow API
}

#[derive(Clone, Deserialize)]
pub struct WowAuth {
    pub access_token: String,
    pub expires_at: SystemTime,
}

impl TypeMapKey for Google {
    type Value = Google;
}

impl TypeMapKey for TarkovMarket {
    type Value = TarkovMarket;
}

impl TypeMapKey for TomorrowIO {
    type Value = TomorrowIO;
}

impl TypeMapKey for Twitch {
    type Value = Twitch;
}

impl TypeMapKey for Wow {
    type Value = Wow;
}

#[derive(Deserialize)]
pub struct Main {
    pub owner_id: u64,
    pub discord: Discord,
    pub google: Google,
    pub psql: Psql,
    pub tarkov_market: TarkovMarket,
    pub tomorrow_io: TomorrowIO,
    pub twitch: Twitch,
    pub wow: Wow,
}
