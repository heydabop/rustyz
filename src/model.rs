use serde::Deserialize;
use serenity::model::{
    id::{GuildId, UserId},
    user::OnlineStatus,
};
use serenity::prelude::*;
use sqlx::{Pool, Postgres};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

pub struct OldDB;

impl TypeMapKey for OldDB {
    type Value = Pool<Postgres>;
}

pub struct DB;

impl TypeMapKey for DB {
    type Value = Pool<Postgres>;
}

pub struct OwnerId;

impl TypeMapKey for OwnerId {
    type Value = u64;
}

pub struct UserPresence {
    pub status: OnlineStatus,
    pub game_name: Option<String>,
}

pub struct LastUserPresence;

impl TypeMapKey for LastUserPresence {
    type Value = Arc<RwLock<HashMap<UserId, UserPresence>>>;
}

pub struct UserGuildList;

impl TypeMapKey for UserGuildList {
    type Value = Arc<RwLock<HashMap<UserId, HashSet<GuildId>>>>;
}

pub struct GuildVoiceLocks;

impl TypeMapKey for GuildVoiceLocks {
    type Value = Arc<Mutex<HashMap<GuildId, Arc<Mutex<()>>>>>;
}

#[derive(Deserialize, Clone, Copy)]
pub struct Point {
    pub lat: f64,
    pub lng: f64,
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.lat, self.lng)
    }
}

pub struct StartInstant;

impl TypeMapKey for StartInstant {
    type Value = Instant;
}
