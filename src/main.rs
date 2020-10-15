#![deny(clippy::all)]
#![warn(clippy::pedantic)]

mod commands;
mod util;

use commands::{ping::PING_COMMAND, top::TOP_COMMAND, wow::CHARACTER_COMMAND};
use serde::Deserialize;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::{
    macros::{group, hook},
    CommandError, StandardFramework,
};
use serenity::model::{channel::Message, gateway::Ready, id::UserId};
use serenity::prelude::*;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::time::SystemTime;

struct DB;

impl TypeMapKey for DB {
    type Value = Pool<Postgres>;
}

#[derive(Deserialize)]
struct DiscordConfig {
    bot_token: String,
    user_id: u64,
}

#[derive(Deserialize)]
struct PsqlConfig {
    url: String,
}

#[derive(Clone, Deserialize)]
struct WowAuth {
    access_token: String,
    expires_at: SystemTime,
}

#[derive(Clone, Deserialize)]
struct WowConfig {
    client_id: String,
    client_secret: String,
    auth: Option<WowAuth>, // not populated by config.toml, populated by first request to wow API
}

impl TypeMapKey for WowConfig {
    type Value = WowConfig;
}

#[derive(Deserialize)]
struct Config {
    discord: DiscordConfig,
    psql: PsqlConfig,
    wow: WowConfig,
}

#[group]
#[commands(ping, top)]
struct General;

#[group]
#[prefix = "wow"]
#[commands(character)]
struct Wow;

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

#[hook]
async fn after_log_error(
    ctx: &Context,
    msg: &Message,
    cmd_name: &str,
    error: Result<(), CommandError>,
) {
    if let Err(why) = error {
        println!(
            "Error in {}: {:?}\n\tMessage: {}",
            cmd_name, why, msg.content
        );
        if let Err(e) = msg.channel_id.say(&ctx.http, "Something broke").await {
            println!("Error sending error reply: {}", e);
        };
    }
}

#[tokio::main]
async fn main() {
    let config: Config =
        toml::from_str(&std::fs::read_to_string("config.toml").expect("Error loading config.toml"))
            .expect("Error parsing config.toml");

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(config.psql.url.as_str())
        .await
        .expect("Error connecting to PSQL database");

    let framework = StandardFramework::new()
        .configure(|c| {
            c.prefix("\\")
                .with_whitespace(true)
                .on_mention(Some(UserId(config.discord.user_id)))
                .no_dm_prefix(true)
                .case_insensitivity(true)
        })
        .group(&GENERAL_GROUP)
        .group(&WOW_GROUP)
        .before(before_typing)
        .after(after_log_error);
    let mut client = Client::new(config.discord.bot_token)
        .type_map_insert::<DB>(pool)
        .type_map_insert::<WowConfig>(config.wow)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating Discord client");

    println!("Starting...");

    if let Err(e) = client.start().await {
        println!("Error running Discord client: {:?}", e);
    }
}
