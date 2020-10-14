#![deny(clippy::all)]
#![warn(clippy::pedantic)]

mod commands;

use commands::{
    ping::PING_COMMAND,
    top::TOP_COMMAND
};
use serde::Deserialize;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::{
    macros::{group, hook},
    StandardFramework,
};
use serenity::model::{channel::Message, gateway::Ready, id::UserId};
use serenity::prelude::*;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

struct DB;

impl TypeMapKey for DB {
    type Value = Pool<Postgres>;
}

#[derive(Deserialize)]
struct Config {
    discord_token: String,
    discord_user_id: u64,
    psql_url: String,
}

#[group]
#[commands(ping, top)]
struct General;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Bot {} is successfully connected.", ready.user.name);
    }
}

#[hook]
async fn before_typing(ctx: &Context, msg: &Message, _: &str) -> bool {
    let http = ctx.http.clone();
    let channel_id = msg.channel_id.0;
    tokio::spawn(async move {
        let _ = http.broadcast_typing(channel_id).await;
    });
    true
}

#[tokio::main]
async fn main() {
    let config: Config =
        toml::from_str(&std::fs::read_to_string("config.toml").expect("Error loading config.toml"))
            .expect("Error parsing config.toml");

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(config.psql_url.as_str())
        .await
        .expect("Error connecting to PSQL database");

    let framework = StandardFramework::new()
        .configure(|c| {
            c.prefix("\\")
                .with_whitespace(true)
                .on_mention(Some(UserId(config.discord_user_id)))
                .no_dm_prefix(true)
                .case_insensitivity(true)
        })
        .group(&GENERAL_GROUP)
        .before(before_typing);
    let mut client = Client::new(config.discord_token)
        .type_map_insert::<DB>(pool)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating Discord client");

    println!("Starting...");

    if let Err(e) = client.start().await {
        println!("Error running Discord client: {:?}", e);
    }
}
