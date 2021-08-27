use serenity::model::{
    id::{ChannelId, MessageId, UserId},
    user::OnlineStatus,
};
use serenity::prelude::*;
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::sync::Arc;

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

pub struct LastCommandMessages;

#[allow(clippy::type_complexity)]
impl TypeMapKey for LastCommandMessages {
    type Value = Arc<RwLock<HashMap<(ChannelId, UserId), [MessageId; 2]>>>;
}

pub struct LastUserPresence;

impl TypeMapKey for LastUserPresence {
    type Value = HashMap<UserId, UserPresence>;
}
