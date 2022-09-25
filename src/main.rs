mod commands;
mod config;
mod event;
mod google;
mod model;
mod shippo;
mod tomorrowio;
mod twitch;
mod util;

use serenity::client::Client;
use serenity::framework::standard::StandardFramework;
use serenity::http::client::Http;
use serenity::model::gateway::GatewayIntents;
use serenity::prelude::*;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, Pool, Postgres};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{error, info};
use warp::{http::StatusCode, Filter};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cfg: config::Main = match std::fs::read_to_string("config.toml") {
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
        options.disable_statement_logging();

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
    let shippo_pool = pool.clone();

    let framework = StandardFramework::new();
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = match Client::builder(cfg.discord.bot_token, intents)
        .application_id(cfg.discord.application_id)
        .type_map_insert::<model::OldDB>(old_pool)
        .type_map_insert::<model::DB>(pool)
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
        .event_handler(event::Handler::default())
        .framework(framework)
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

    set.spawn(async move {
        let addr: std::net::SocketAddr = ([127, 0, 0, 1], 8125).into();
        let health = warp::path("health")
            .map(warp::reply)
            .map(|reply| warp::reply::with_status(reply, StatusCode::NO_CONTENT));
        let shippo_tracking = warp::post()
            .and(warp::path!("shippo" / "tracking"))
            .and(warp::body::json())
            .and(with_db(shippo_pool))
            .and(with_http(shippo_http))
            .and_then(shippo::handle_post);
        warp::serve(health.or(shippo_tracking)).run(addr).await;
    });

    set.spawn(async move {
        if let Err(e) = client.start().await {
            error!(%e, "Error running Discord client");
        }
    });

    if let Some(Err(e)) = set.join_next().await {
        error!(%e, "Error joining task");
    }

    info!("Exiting");
}

fn with_db(
    db: Pool<Postgres>,
) -> impl Filter<Extract = (Pool<Postgres>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}

fn with_http(
    http: Arc<Http>,
) -> impl Filter<Extract = (Arc<Http>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || http.clone())
}
