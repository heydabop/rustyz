mod commands;
mod config;
mod error;
mod event;
mod google;
mod model;
mod shippo;
mod tomorrowio;
mod twitch;
mod util;

use log::LevelFilter;
use serenity::client::Client;
use serenity::model::gateway::GatewayIntents;
use serenity::prelude::*;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cfg: config::Main = match tokio::fs::read_to_string("config.toml").await {
        Ok(s) => match toml::from_str(&s) {
            Ok(t) => t,
            Err(e) => {
                error!(%e, "Error parsing config.toml");
                return;
            }
        },
        Err(e) => {
            error!(%e, "Error loading config.toml");
            return;
        }
    };

    let old_pool = {
        let mut old_options = match PgConnectOptions::from_str(cfg.psql.old_url.as_str()) {
            Ok(s) => s,
            Err(e) => {
                error!(%e, "Error parsing old DB connection string");
                return;
            }
        };
        old_options.disable_statement_logging();

        match PgPoolOptions::new()
            .min_connections(1)
            .max_connections(4)
            .connect_with(old_options)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                error!(%e, "Error connecting to old PSQL database");
                return;
            }
        }
    };

    let pool = {
        let mut options = match PgConnectOptions::from_str(cfg.psql.url.as_str()) {
            Ok(s) => s,
            Err(e) => {
                error!(%e, "Error parsing DB connection string");
                return;
            }
        };
        options.log_statements(LevelFilter::Trace);
        options.log_slow_statements(LevelFilter::Warn, Duration::from_secs_f32(0.5));

        match PgPoolOptions::new()
            .min_connections(1)
            .max_connections(4)
            .connect_with(options)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                error!(%e, "Error connecting to PSQL database");
                return;
            }
        }
    };

    let shippo_api_key = cfg.shippo.api_key.clone();

    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = match Client::builder(cfg.discord.bot_token, intents)
        .application_id(cfg.discord.application_id)
        .type_map_insert::<model::OldDB>(old_pool)
        .type_map_insert::<model::DB>(pool.clone())
        .type_map_insert::<config::Google>(cfg.google)
        .type_map_insert::<config::Shippo>(cfg.shippo)
        .type_map_insert::<config::TarkovMarket>(cfg.tarkov_market)
        .type_map_insert::<config::TomorrowIO>(cfg.tomorrow_io)
        .type_map_insert::<config::Twitch>(cfg.twitch)
        .type_map_insert::<config::Wow>(cfg.wow)
        .type_map_insert::<model::OwnerId>(cfg.owner_id)
        .type_map_insert::<model::LastUserPresence>(Arc::new(RwLock::new(HashMap::new())))
        .type_map_insert::<model::UserGuildList>(Arc::new(RwLock::new(HashMap::new())))
        .type_map_insert::<model::LastCommandMessages>(Arc::new(RwLock::new(HashMap::new())))
        .type_map_insert::<model::StartInstant>(Instant::now())
        .event_handler(event::Handler::new(pool.clone()))
        .await
    {
        Ok(c) => c,
        Err(e) => {
            error!(%e, "Error creating Discord client");
            return;
        }
    };

    info!("Starting...");

    let mut set = JoinSet::new();
    let shippo_http = client.cache_and_http.clone().http.clone();

    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!(error = %e, "error setting sigint handler");
        }
        shard_manager.lock().await.shutdown_all().await;
    });

    set.spawn(async move {
        if let Err(e) = client.start().await {
            error!(%e, "Error running Discord client");
        }
    });

    set.spawn(shippo::poll_shipments_loop(
        shippo_http,
        pool,
        shippo_api_key,
    ));

    if let Some(Err(e)) = set.join_next().await {
        error!(%e, "Error joining task");
    }

    info!("Exiting");
}
