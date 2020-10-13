use serde::Deserialize;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult, StandardFramework,
};
use serenity::model::{channel::Message, gateway::Ready};

#[derive(Deserialize)]
struct Config {
    discord_token: String,
}

#[group]
#[commands(ping)]
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
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("\\"))
        .group(&GENERAL_GROUP);

    let config: Config =
        toml::from_str(&std::fs::read_to_string("config.toml").expect("Error loading config.toml"))
            .expect("Error parsing config.toml");
    let mut client = Client::new(config.discord_token)
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
