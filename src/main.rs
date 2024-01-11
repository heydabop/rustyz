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
use serenity::all::ApplicationId;
use serenity::client::Client;
use serenity::model::gateway::GatewayIntents;
use serenity::prelude::*;
use songbird::SerenityInit;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, Pool, Postgres};
use std::collections::HashMap;
use std::process::exit;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::Mutex;
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
                exit(1);
            }
        },
        Err(e) => {
            error!(%e, "Error loading config.toml");
            exit(1);
        }
    };

    let mut sigint = match signal(SignalKind::interrupt()) {
        Ok(s) => s,
        Err(e) => {
            error!(%e, "Error registering SIGINT handler");
            exit(1);
        }
    };
    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(e) => {
            error!(%e, "Error registering SIGTERM handler");
            exit(1);
        }
    };

    let pool = {
        let mut options = match PgConnectOptions::from_str(cfg.psql.url.as_str()) {
            Ok(s) => s,
            Err(e) => {
                error!(%e, "Error parsing DB connection string");
                exit(1);
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
                exit(1);
            }
        }
    };

    let db_conn = pool.clone();

    let shippo_api_key = cfg.shippo.api_key.clone();

    let event_handler = match event::Handler::new(pool.clone()) {
        Ok(h) => h,
        Err(e) => {
            error!(%e, "Error creating event handler");
            exit(1);
        }
    };

    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = match Client::builder(cfg.discord.bot_token, intents)
        .application_id(ApplicationId::new(cfg.discord.application_id))
        .type_map_insert::<model::DB>(pool.clone())
        .type_map_insert::<config::Google>(cfg.google)
        .type_map_insert::<config::Shippo>(cfg.shippo)
        .type_map_insert::<config::TarkovMarket>(cfg.tarkov_market)
        .type_map_insert::<config::TomorrowIO>(cfg.tomorrow_io)
        .type_map_insert::<config::Twitch>(cfg.twitch)
        .type_map_insert::<config::WolframAlpha>(cfg.wolfram_alpha)
        .type_map_insert::<config::Wow>(cfg.wow)
        .type_map_insert::<model::OwnerId>(cfg.owner_id)
        .type_map_insert::<model::LastUserPresence>(Arc::new(RwLock::new(HashMap::new())))
        .type_map_insert::<model::UserGuildList>(Arc::new(RwLock::new(HashMap::new())))
        .type_map_insert::<model::StartInstant>(Instant::now())
        .type_map_insert::<model::GuildVoiceLocks>(Arc::new(Mutex::new(HashMap::new())))
        .event_handler(event_handler)
        .register_songbird()
        .await
    {
        Ok(c) => c,
        Err(e) => {
            error!(%e, "Error creating Discord client");
            exit(1);
        }
    };

    info!("Starting...");

    let mut set = JoinSet::new();
    let shippo_http = client.http.clone();

    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = sigint.recv() => {},
            _ = sigterm.recv() => {},
        };
        shard_manager.shutdown_all().await;
    });

    set.spawn(async move {
        if let Err(e) = client.start().await {
            error!(%e, "Error running Discord client");
            exit(1);
        }
    });

    set.spawn(shippo::poll_shipments_loop(
        shippo_http,
        pool,
        shippo_api_key,
    ));

    let start_id =
        match sqlx::query!("INSERT INTO bot_start(clean_shutdown) VALUES (false) RETURNING id")
            .fetch_one(&db_conn)
            .await
        {
            Ok(r) => Some(r.id),
            Err(e) => {
                error!(%e, "Error inserting into bot_start");
                None
            }
        };

    let updater_conn = db_conn.clone();
    if let Some(start_id) = start_id {
        set.spawn(uptime_update_loop(updater_conn.clone(), start_id));
    }

    if let Some(Err(e)) = set.join_next().await {
        error!(%e, "Error joining task");
        exit(1);
    }

    if let Some(start_id) = start_id {
        #[allow(clippy::panic)]
        if let Err(e) = sqlx::query!(
            "UPDATE bot_start SET update_date = now(), clean_shutdown = true WHERE id = $1",
            start_id
        )
        .execute(&db_conn)
        .await
        {
            error!(%e, "Error updating bot_start");
        }
    }

    info!("Exiting");
}

async fn uptime_update_loop(db: Pool<Postgres>, start_id: i32) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
    loop {
        interval.tick().await;

        #[allow(clippy::panic)]
        if let Err(e) = sqlx::query!(
            "UPDATE bot_start SET update_date = now() WHERE id = $1",
            start_id
        )
        .execute(&db)
        .await
        {
            error!(%e, "Error updating bot_start");
        }
    }
}
