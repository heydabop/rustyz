#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::too_many_lines)]

mod commands;
mod util;

use chrono::prelude::*;
use commands::{
    affixes::AFFIXES_COMMAND,
    delete::DELETE_COMMAND,
    fortune::FORTUNE_COMMAND,
    karma::KARMA_COMMAND,
    ping::PING_COMMAND,
    playtime::{gen_playtime_message, PLAYTIME_COMMAND, RECENT_PLAYTIME_COMMAND},
    raiderio::RAIDERIO_COMMAND,
    source::SOURCE_COMMAND,
    tarkov::TARKOV_COMMAND,
    top::TOP_COMMAND,
    whois::WHOIS_COMMAND,
    wow::CHARACTER_COMMAND,
    wow::MOG_COMMAND,
    wow::REALM_COMMAND,
    wow::SEARCH_COMMAND,
};
use serde::Deserialize;
use serenity::async_trait;
use serenity::client::{bridge::gateway::GatewayIntents, Client, Context, EventHandler};
use serenity::framework::standard::{
    help_commands,
    macros::{group, help, hook},
    Args, CommandError, CommandGroup, CommandResult, HelpOptions, StandardFramework,
};
use serenity::model::{
    channel::Message,
    event::PresenceUpdateEvent,
    gateway::{ActivityType, Ready},
    id::{ChannelId, MessageId, UserId},
    interactions::{
        message_component::{ButtonStyle, InteractionMessage},
        Interaction, InteractionApplicationCommandCallbackDataFlags, InteractionResponseType,
    },
    user::OnlineStatus,
};
use serenity::prelude::*;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres, Row};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::SystemTime;

struct OldDB;

impl TypeMapKey for OldDB {
    type Value = Pool<Postgres>;
}

struct DB;

impl TypeMapKey for DB {
    type Value = Pool<Postgres>;
}

struct OwnerId;

impl TypeMapKey for OwnerId {
    type Value = u64;
}

struct UserPresence {
    status: OnlineStatus,
    game_name: Option<String>,
}

struct LastUserPresence;

impl TypeMapKey for LastUserPresence {
    type Value = HashMap<UserId, UserPresence>;
}

struct LastCommandMessages;

#[allow(clippy::type_complexity)]
impl TypeMapKey for LastCommandMessages {
    type Value = Arc<RwLock<HashMap<(ChannelId, UserId), [MessageId; 2]>>>;
}

#[derive(Deserialize)]
struct DiscordConfig {
    application_id: u64,
    bot_token: String,
    user_id: u64,
}

#[derive(Deserialize)]
struct PsqlConfig {
    old_url: String,
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

#[derive(Deserialize)]
struct TarkovMarketConfig {
    api_key: String,
}

impl TypeMapKey for TarkovMarketConfig {
    type Value = TarkovMarketConfig;
}

impl TypeMapKey for WowConfig {
    type Value = WowConfig;
}

#[derive(Deserialize)]
struct Config {
    owner_id: u64,
    discord: DiscordConfig,
    psql: PsqlConfig,
    tarkov_market: TarkovMarketConfig,
    wow: WowConfig,
}

const FAST_COMMANDS: [&str; 4] = ["delete", "fortune", "ping", "source"];

#[group]
#[commands(
    affixes,
    delete,
    fortune,
    karma,
    ping,
    playtime,
    recent_playtime,
    source,
    tarkov,
    top,
    raiderio,
    whois
)]
struct General;

#[group]
#[prefix = "wow"]
#[commands(character, realm, search, mog)]
struct Wow;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Bot {} is successfully connected.", ready.user.name);
    }

    async fn presence_update(&self, ctx: Context, update: PresenceUpdateEvent) {
        let presence = update.presence;
        let user_id = presence.user_id;
        if match presence.user {
            Some(user) => user.bot,
            None => match ctx.cache.user(user_id).await {
                Some(user) => user.bot,
                None => {
                    if let Ok(user) = ctx.http.get_user(user_id.0).await {
                        user.bot
                    } else {
                        println!("Unable to determine if user {} is bot", user_id);
                        false
                    }
                }
            },
        } {
            // ignore updates from bots
            return;
        }
        let game_name = presence.activities.iter().find_map(|a| {
            if a.kind == ActivityType::Playing {
                // clients reporting ® and ™ seems inconsistent, so the same game gets different names overtime
                let mut game_name = a.name.replace(&['®', '™'][..], "");
                game_name.truncate(512);
                Some(game_name)
            } else {
                None
            }
        });

        {
            let data = ctx.data.read().await;
            if let Some(last_presence) = data.get::<LastUserPresence>().unwrap().get(&user_id) {
                if last_presence.status == presence.status && last_presence.game_name == game_name {
                    return;
                }
            }
        }

        let mut data = ctx.data.write().await;
        let db = data.get::<DB>().unwrap();
        #[allow(clippy::cast_possible_wrap)] if let Err(e) = sqlx::query(
            r#"INSERT INTO user_presence (user_id, status, game_name) VALUES ($1, $2::online_status, $3)"#,
        )
        .bind(user_id.0 as i64)
            .bind(presence.status.name())
            .bind(&game_name)
            .execute(&*db)
            .await
        {
            println!("Error saving user_presence: {}", e);
            return;
        }
        let last_presence_map = data.get_mut::<LastUserPresence>().unwrap();
        last_presence_map.insert(
            user_id,
            UserPresence {
                status: presence.status,
                game_name,
            },
        );
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let interaction = match interaction.message_component() {
            Some(c) => c,
            None => return,
        };
        let fields: Vec<&str> = interaction.data.custom_id.split(':').collect();
        let command = fields[0];
        if command != "playtime" {
            return;
        }
        let prev_next = fields[1];
        let button_id = fields[2].parse::<i64>().unwrap();
        let row = {
            let data = ctx.data.read().await;
            let db = data.get::<DB>().unwrap();
            match sqlx::query(r#"SELECT author_id, user_ids, username, end_date, start_offset FROM playtime_button WHERE id = $1"#).bind(button_id).fetch_one(&*db).await {
                Ok(row) => row,
                Err(e) => {println!("{}", e);return;}
            }
        };
        let author_id = row.get::<i64, _>(0);

        #[allow(clippy::cast_possible_wrap)]
        if author_id != interaction.user.id.0 as i64 {
            if let Err(e) = interaction
                .create_interaction_response(ctx, |r| {
                    r.interaction_response_data(|d| {
                        d.flags(InteractionApplicationCommandCallbackDataFlags::EPHEMERAL);
                        d.content("Sorry, only the original command user can change the message");
                        d
                    });
                    r
                })
                .await
            {
                println!("{}", e);
            }
            return;
        }

        let user_ids = row.get::<Vec<i64>, _>(1);
        let username = row.get::<Option<String>, _>(2);
        let end_date = row.get::<DateTime<FixedOffset>, _>(3);
        let mut offset = row.get::<i32, _>(4);

        if prev_next == "prev" {
            offset = (offset - 10).max(0);
        } else if prev_next == "next" {
            offset += 10;
        } else {
            return;
        }

        #[allow(clippy::cast_sign_loss)]
        let new_content =
            gen_playtime_message(&ctx, &user_ids, &username, None, end_date, offset as usize)
                .await
                .unwrap();
        let mut message = match interaction.message {
            InteractionMessage::Regular(ref m) => m.clone(),
            InteractionMessage::Ephemeral(_) => return,
        };
        if let Err(e) = message
            .edit(&ctx, |m| {
                m.content(&new_content);
                m.components(|c| {
                    c.create_action_row(|a| {
                        a.create_button(|b| {
                            b.custom_id(format!("playtime:prev:{}", button_id))
                                .style(ButtonStyle::Primary)
                                .label("Prev 10")
                                .disabled(offset < 1);
                            b
                        });
                        a.create_button(|b| {
                            b.custom_id(format!("playtime:next:{}", button_id))
                                .style(ButtonStyle::Primary)
                                .label("Next 10")
                                .disabled(new_content.matches('\n').count() < 12);
                            b
                        });
                        a
                    });
                    c
                });
                m
            })
            .await
        {
            println!("{}", e);
            return;
        }

        if let Err(e) = interaction
            .create_interaction_response(&ctx, |r| {
                r.kind(InteractionResponseType::UpdateMessage);
                r
            })
            .await
        {
            println!("{}", e);
            return;
        }

        {
            let data = ctx.data.read().await;
            let db = data.get::<DB>().unwrap();
            if let Err(e) =
                sqlx::query(r#"UPDATE playtime_button SET start_offset = $2 WHERE id = $1"#)
                    .bind(button_id)
                    .bind(offset)
                    .execute(&*db)
                    .await
            {
                println!("{}", e);
            }
        }
    }
}

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
            *(data.get::<OwnerId>().unwrap())
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
    let config: Config =
        toml::from_str(&std::fs::read_to_string("config.toml").expect("Error loading config.toml"))
            .expect("Error parsing config.toml");

    let old_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(config.psql.old_url.as_str())
        .await
        .expect("Error connecting to old PSQL database");

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(config.psql.url.as_str())
        .await
        .expect("Error connecting to PSQL database");

    let framework = StandardFramework::new()
        .configure(|c| {
            c.prefix("!")
                .with_whitespace(true)
                .on_mention(Some(UserId(config.discord.user_id)))
                .no_dm_prefix(true)
                .case_insensitivity(true)
        })
        .group(&GENERAL_GROUP)
        .group(&WOW_GROUP)
        .help(&DEFAULT_HELP)
        .before(before_typing)
        .after(after_log_error);
    let mut client = Client::builder(config.discord.bot_token)
        .application_id(config.discord.application_id)
        .type_map_insert::<OldDB>(old_pool)
        .type_map_insert::<DB>(pool)
        .type_map_insert::<TarkovMarketConfig>(config.tarkov_market)
        .type_map_insert::<WowConfig>(config.wow)
        .type_map_insert::<OwnerId>(config.owner_id)
        .type_map_insert::<LastUserPresence>(HashMap::new())
        .type_map_insert::<LastCommandMessages>(Arc::new(RwLock::new(HashMap::new())))
        .event_handler(Handler)
        .intents(
            GatewayIntents::GUILD_MEMBERS
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
