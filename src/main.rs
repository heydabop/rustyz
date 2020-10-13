use num_traits::cast::ToPrimitive;
use serde::Deserialize;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult, StandardFramework,
};
use serenity::model::{channel::Message, gateway::Ready};
use serenity::prelude::*;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres, Row};
use sqlx::types::Decimal;

struct DB;

impl TypeMapKey for DB {
    type Value = Pool<Postgres>;
}

#[derive(Deserialize)]
struct Config {
    discord_token: String,
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
        .configure(|c| c.prefix("\\"))
        .group(&GENERAL_GROUP);
    let mut client = Client::new(config.discord_token)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating Discord client");
    {
        let mut data = client.data.write().await;
        data.insert::<DB>(pool);
    }

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
async fn top(ctx: &Context, msg: &Message) -> CommandResult {
    let limit: i32 = 5;
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

    for row in &rows {
        let user_id = row.get::<Decimal, _>(0).to_u64().unwrap();
        let num_messages: i64 = row.get(1);
        let mut username = String::from("`<UNKNOWN>`");
        if let Ok(user) = ctx.http.get_user(user_id).await {
            username = user.name.clone();
            if let Some(guild_id) = msg.guild_id {
                if let Some(nick) = user.nick_in(&ctx.http, guild_id).await {
                    username = nick.clone();
                }
            }
        }
        lines.push(format!(
            "{} \u{2014} {}\n",
            username,
            num_messages
        ));
    }

    msg.channel_id.say(&ctx.http, lines.concat()).await?;

    Ok(())
}
