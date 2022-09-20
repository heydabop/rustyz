#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::too_many_lines,
    clippy::module_name_repetitions,
    clippy::enum_glob_use
)]

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
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;
use warp::{http::StatusCode, Filter};

#[tokio::main]
async fn main() {
    let cfg: config::Main =
        toml::from_str(&std::fs::read_to_string("config.toml").expect("Error loading config.toml"))
            .expect("Error parsing config.toml");

    let old_pool = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(4)
        .connect(cfg.psql.old_url.as_str())
        .await
        .expect("Error connecting to old PSQL database");

    let pool = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(4)
        .connect(cfg.psql.url.as_str())
        .await
        .expect("Error connecting to PSQL database");
    let shippo_pool = pool.clone();

    let framework = StandardFramework::new();
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(cfg.discord.bot_token, intents)
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
        .expect("Error creating Discord client");

    println!("Starting...");

    let mut set = JoinSet::new();
    let shippo_http = client.cache_and_http.clone().http.clone();

    set.spawn(async move {
        let addr: std::net::SocketAddr = ([127, 0, 0, 1], 8125).into();
        println!("HTTP listening on {:?}", addr);
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
            eprintln!("Error running Discord client: {:?}", e);
        }
    });

    if let Some(Err(e)) = set.join_next().await {
        eprintln!("Error joining task: {:?}", e);
    }

    println!("Exiting");
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
