#![deny(clippy::all)]
#![warn(clippy::pedantic)]

use num_traits::cast::ToPrimitive;
use serde::Deserialize;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::{
    macros::{command, group, hook},
    Args, CommandResult, StandardFramework,
};
use serenity::model::{channel::Message, gateway::Ready, id::UserId};
use serenity::prelude::*;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Decimal;
use sqlx::{Pool, Postgres, Row};

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

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, "pong").await?;

    Ok(())
}

#[command]
async fn top(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let limit: i32 = args.single().unwrap_or(5);
    let chan_id = Decimal::from(msg.channel_id.0);

    let data = ctx.data.read().await;
    let db = data.get::<DB>().unwrap();
    let rows = sqlx::query(
        r#"
	SELECT author_id, count(author_id) AS num_messages
	FROM message
	WHERE chan_id = $1
	AND content NOT LIKE '/%'
	GROUP BY author_id
	ORDER BY count(author_id) DESC
	LIMIT $2"#,
    )
    .bind(chan_id)
    .bind(limit)
    .fetch_all(&*db)
    .await
    .unwrap();

    let mut lines = vec![];
    let guild_id = msg.guild_id.unwrap();

    for row in &rows {
        let user_id = row.get::<Decimal, _>(0).to_u64().unwrap();
        let num_messages: i64 = row.get(1);
        let username = {
            match ctx
                .cache
                .member_field(guild_id, user_id, |m| m.nick.clone())
                .await
            {
                Some(nick) if nick.is_some() => nick.unwrap(),
                _ => match ctx.http.get_member(guild_id.0, user_id).await {
                    Ok(member) if member.nick.is_some() => member.nick.unwrap(),
                    _ => match ctx.cache.user(user_id).await {
                        Some(user) => user.name,
                        None => match ctx.http.get_user(user_id).await {
                            Ok(user) => user.name,
                            Err(_) => String::from("`<UNKNOWN`>"),
                        },
                    },
                },
            }
        };
        println!("{}ms", ms);
        lines.push(format!("{} \u{2014} {}\n", username, num_messages));
    }

    msg.channel_id.say(&ctx.http, lines.concat()).await?;

    Ok(())
}
