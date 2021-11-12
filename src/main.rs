#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::too_many_lines)]

mod commands;
mod config;
mod event;
mod google;
mod model;
mod tomorrowio;
mod util;

use commands::{
    affixes::AFFIXES_COMMAND,
    delete::DELETE_COMMAND,
    fortune::FORTUNE_COMMAND,
    karma::KARMA_COMMAND,
    lastseen::LASTSEEN_COMMAND,
    ping::PING_COMMAND,
    playtime::{PLAYTIME_COMMAND, RECENT_PLAYTIME_COMMAND},
    raiderio::RAIDERIO_COMMAND,
    source::SOURCE_COMMAND,
    tarkov::TARKOV_COMMAND,
    time::{
        BIRDTIME_COMMAND, MIROTIME_COMMAND, NIELTIME_COMMAND, SEBBITIME_COMMAND, USTIME_COMMAND,
    },
    top::TOP_COMMAND,
    toplength::TOPLENGTH_COMMAND,
    weather::WEATHER_COMMAND,
    whois::WHOIS_COMMAND,
    wow::CHARACTER_COMMAND,
    wow::MOG_COMMAND,
    wow::REALM_COMMAND,
    wow::SEARCH_COMMAND,
};
use serenity::client::{bridge::gateway::GatewayIntents, Client, Context};
use serenity::framework::standard::{
    help_commands,
    macros::{group, help, hook},
    Args, CommandError, CommandGroup, CommandResult, HelpOptions, StandardFramework,
};
use serenity::model::{channel::Message, id::UserId};
use serenity::prelude::*;
use sqlx::postgres::PgPoolOptions;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const FAST_COMMANDS: [&str; 5] = ["delete", "fortune", "lastseen", "ping", "source"];

#[group]
#[commands(
    affixes,
    birdtime,
    delete,
    fortune,
    lastseen,
    karma,
    mirotime,
    nieltime,
    ping,
    playtime,
    recent_playtime,
    sebbitime,
    source,
    tarkov,
    top,
    toplength,
    ustime,
    raiderio,
    weather,
    whois
)]
struct General;

#[group]
#[prefix = "wow"]
#[commands(character, realm, search, mog)]
struct Wow;

#[hook]
async fn before_typing(ctx: &Context, msg: &Message, cmd: &str) -> bool {
    if FAST_COMMANDS.contains(&cmd) {
        // fast running commands dont need to broadcast typing
        return true;
    }
    let http = ctx.http.clone();
    let channel_id = msg.channel_id.0;
    tokio::spawn(async move {
        std::mem::drop(http.broadcast_typing(channel_id).await);
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
        let error_message = format!(
            "Error in {}: {:?}\n\tMessage: {}",
            cmd_name, why, msg.content
        );
        println!("{}", error_message);
        if let Err(e) = util::record_say(ctx, msg, "Something broke").await {
            println!("Error sending error reply: {}", e);
        };
        let owner_id: u64 = {
            let data = ctx.data.read().await;
            *(data.get::<model::OwnerId>().unwrap())
        };
        let owner = match ctx.cache.user(owner_id).await {
            Some(owner) => owner,
            None => ctx.http.get_user(owner_id).await.unwrap(),
        };
        if let Err(e) = owner
            .direct_message(&ctx.http, |m| {
                m.content(error_message);
                m
            })
            .await
        {
            println!("Error sending error DM: {}", e);
        }
    }
}

#[help]
#[strikethrough_commands_tip_in_guild("")]
async fn default_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _help = help_commands::plain(context, msg, args, help_options, groups, owners).await;
    Ok(())
}

#[tokio::main]
async fn main() {
    let cfg: config::Main =
        toml::from_str(&std::fs::read_to_string("config.toml").expect("Error loading config.toml"))
            .expect("Error parsing config.toml");

    let old_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(cfg.psql.old_url.as_str())
        .await
        .expect("Error connecting to old PSQL database");

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(cfg.psql.url.as_str())
        .await
        .expect("Error connecting to PSQL database");

    let framework = StandardFramework::new()
        .configure(|c| {
            c.prefix("/")
                .with_whitespace(true)
                .on_mention(Some(UserId(cfg.discord.user_id)))
                .no_dm_prefix(true)
                .case_insensitivity(true)
        })
        .group(&GENERAL_GROUP)
        .group(&WOW_GROUP)
        .help(&DEFAULT_HELP)
        .before(before_typing)
        .after(after_log_error);
    let mut client = Client::builder(cfg.discord.bot_token)
        .application_id(cfg.discord.application_id)
        .type_map_insert::<model::OldDB>(old_pool)
        .type_map_insert::<model::DB>(pool)
        .type_map_insert::<config::Google>(cfg.google)
        .type_map_insert::<config::TarkovMarket>(cfg.tarkov_market)
        .type_map_insert::<config::TomorrowIO>(cfg.tomorrow_io)
        .type_map_insert::<config::Wow>(cfg.wow)
        .type_map_insert::<model::OwnerId>(cfg.owner_id)
        .type_map_insert::<model::LastUserPresence>(Arc::new(RwLock::new(HashMap::new())))
        .type_map_insert::<model::UserGuildList>(Arc::new(RwLock::new(HashMap::new())))
        .type_map_insert::<model::LastCommandMessages>(Arc::new(RwLock::new(HashMap::new())))
        .event_handler(event::Handler)
        .intents(
            GatewayIntents::GUILDS
                | GatewayIntents::GUILD_MEMBERS
                | GatewayIntents::GUILD_PRESENCES
                | GatewayIntents::GUILD_MESSAGES
                | GatewayIntents::DIRECT_MESSAGES,
        )
        .framework(framework)
        .await
        .expect("Error creating Discord client");

    println!("Starting...");

    if let Err(e) = client.start().await {
        println!("Error running Discord client: {:?}", e);
    }
}
