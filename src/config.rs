use crate::model::Point;
use serde::Deserialize;
use serenity::prelude::*;
use std::time::SystemTime;

#[derive(Deserialize)]
pub struct Discord {
    pub application_id: u64,
    pub bot_token: String,
}

#[derive(Deserialize)]
pub struct Google {
    pub maps_api_key: String,
}

#[derive(Deserialize)]
pub struct Psql {
    pub url: String,
}

#[derive(Deserialize)]
pub struct Shippo {
    pub api_key: String,
}

#[derive(Deserialize)]
pub struct TarkovMarket {
    pub api_key: String,
}

#[derive(Clone, Deserialize)]
pub struct TomorrowIO {
    pub api_key: String,
    pub default_location: Point,
    pub default_location_name: String,
}

#[derive(Clone, Deserialize)]
pub struct AirNow {
    pub api_key: String,
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
pub struct WolframAlpha {
    pub app_id: String,
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

impl TypeMapKey for Shippo {
    type Value = Shippo;
}

impl TypeMapKey for TarkovMarket {
    type Value = TarkovMarket;
}

impl TypeMapKey for TomorrowIO {
    type Value = TomorrowIO;
}

impl TypeMapKey for AirNow {
    type Value = AirNow;
}

impl TypeMapKey for Twitch {
    type Value = Twitch;
}

impl TypeMapKey for WolframAlpha {
    type Value = WolframAlpha;
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
    pub shippo: Shippo,
    pub tarkov_market: TarkovMarket,
    pub tomorrow_io: TomorrowIO,
    pub air_now: AirNow,
    pub twitch: Twitch,
    pub wolfram_alpha: WolframAlpha,
    pub wow: Wow,
}
