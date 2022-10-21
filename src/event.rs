use crate::commands;
use crate::model;
use crate::twitch;

use chrono::prelude::*;
use num_format::{Locale, ToFormattedString};
use serde_json::json;
use serenity::async_trait;
use serenity::client::{Context, EventHandler};
use serenity::json::Value;
use serenity::model::{
    application::command::{Command, CommandOptionType},
    application::interaction::{
        application_command::ApplicationCommandInteraction, Interaction, InteractionResponseType,
    },
    channel::{Message, MessageFlags},
    event::MessageUpdateEvent,
    gateway::{ActivityType, Presence, Ready},
    guild::{Guild, Member, UnavailableGuild},
    id::{ChannelId, GuildId, MessageId, UserId},
    user::User,
};
use sqlx::types::Decimal;
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::collections::HashSet;
use tracing::{error, info, warn};

pub struct Handler {
    db: Pool<Postgres>,
    twitch_regex: regex::Regex,
}

impl Handler {
    pub fn new(db: Pool<Postgres>) -> Self {
        #[allow(clippy::unwrap_used)]
        Self {
            db,
            twitch_regex: regex::RegexBuilder::new(r#"https?://(www\.)?twitch.tv/(\w+)"#)
                .case_insensitive(true)
                .build()
                .unwrap(),
        }
    }

    async fn record_command(&self, command: &ApplicationCommandInteraction) {
        let mut command_name = command.data.name.clone();
        let log_options: HashMap<&String, &Option<Value>> =
            if let Some(option) = command.data.options.get(0) {
                if option.kind == CommandOptionType::SubCommand {
                    command_name = format!("{} {}", command_name, option.name);
                    option.options.iter().map(|o| (&o.name, &o.value)).collect()
                } else {
                    command
                        .data
                        .options
                        .iter()
                        .map(|o| (&o.name, &o.value))
                        .collect()
                }
            } else {
                HashMap::new()
            };
        info!(name = command_name, options = ?log_options, "command called");
        let user_id = match i64::try_from(command.user.id.0) {
            Ok(u) => u,
            Err(e) => {
                error!(%e, "unable to fit user id in i64");
                return;
            }
        };
        let channel_id = match i64::try_from(command.channel_id.0) {
            Ok(c) => c,
            Err(e) => {
                error!(%e, "unable to fit channel id in i64");
                return;
            }
        };
        let guild_id = if let Some(g) = command.guild_id {
            match i64::try_from(g.0) {
                Ok(g) => Some(g),
                Err(e) => {
                    error!(%e, "unable to fit guild id in i64");
                    return;
                }
            }
        } else {
            None
        };
        #[allow(clippy::panic)]
        if let Err(e) = sqlx::query!(
            r#"
INSERT INTO command(author_id, channel_id, guild_id, name, options)
VALUES ($1, $2, $3, $4, $5)"#,
            user_id,
            channel_id,
            guild_id,
            command_name,
            json!(log_options)
        )
        .execute(&self.db)
        .await
        {
            error!(%e, "error inserting command log into db");
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Bot {} is successfully connected.", ready.user.name);

        #[allow(clippy::unreadable_literal)]
        let g = GuildId(184428741450006528);
        match g
            .set_application_commands(&ctx.http, |commands| {
                commands
                    .create_application_command(|c| {
                        c.name("birdtime")
                            .description("Sends the current time for bird")
                    })
                    .create_application_command(|c| {
                        c.name("mirotime")
                            .description("Sends the current time for miro")
                    })
                    .create_application_command(|c| {
                        c.name("nieltime")
                            .description("Sends the current time for niel")
                    })
                    .create_application_command(|c| {
                        c.name("realtime")
                            .description("Sends the current time for the mainlanders")
                    })
                    .create_application_command(|c| {
                        c.name("sebbitime")
                            .description("Sends the current time for sebbi")
                    })
            })
            .await
        {
            Ok(guild_commands) => info!(guild_commands = ?guild_commands
                    .iter()
                    .map(|g| &g.name)
                    .collect::<Vec<&String>>(),
                "guild commands set",
            ),
            Err(e) => error!(%e, "error setting guild commands"),
        }

        match Command::set_global_application_commands(&ctx.http, |commands| {
            commands
                .create_application_command(|c| {
                    c.name("affixes").description("Sends this week's US Mythic+ affixes")
                })
                .create_application_command(|c| {
                    c.name("botinfo").description("Displays details about the bot")
                })
                .create_application_command(|c| {
                    c.name("forecast")
                        .description("Sends hourly weather conditions over the next 12 hours for an area")
                        .create_option(|o| {
                            o.name("location")
                                .description("Area to get weather for; can be city name, postal code, or decimal lat/long (default: Austin, TX)")
                                .kind(CommandOptionType::String)
                                .required(false)
                        })
                        .create_option(|o| {
                            o.name("hours")
                                .description("How many hours into the future to forecast (default: 6)")
                                .kind(CommandOptionType::Integer)
                                .required(false)
                                .min_int_value(2)
                                .max_int_value(12)
                        })
                })
                .create_application_command(|c| {
                    c.name("fortune").description("Sends a random adage")
                })
                .create_application_command(|c| {
                    c.name("serverinfo").description("Displays details about this server")
                })
                .create_application_command(|c| {
                    c.name("invite").description("Generates link to add bot to a server you administrate")
                })
                .create_application_command(|c| {
                    c.name("karma")
                        .description("Lists members by karma points")
                        .create_option(|o| {
                            o.name("count")
                                .description("The number of members to list (defaults to 5)")
                                .kind(CommandOptionType::Integer)
                                .required(false)
                                .min_int_value(1)
                                .max_int_value(100)
                        })
                })
                .create_application_command(|c| {
                    c.name("lastseen")
                        .description("Sends how long it's been since a user was last online")
                        .create_option(|o| {
                            o.name("user")
                                .description("User to check")
                                .kind(CommandOptionType::User)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("lastplayed")
                        .description("How long it's been since a user was last playing a game, and the game they were playing")
                        .create_option(|o| {
                            o.name("user")
                                .description("User to check")
                                .kind(CommandOptionType::User)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("ping").description("pong")
                })
                .create_application_command(|c| {
                    c.name("playtime")
                        .description("Shows all recorded video game playtime of a user or everyone in this server")
                        .create_option(|o| {
                            o.name("user")
                                .description("User to show playtime for")
                                .kind(CommandOptionType::User)
                                .required(false)
                        })
                })
                .create_application_command(|c| {
                    c.name("raiderio")
                        .description("Displays raider.io stats for given character")
                        .create_option(|o| {
                            o.name("character")
                                .description("Character to get stats for")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("realm")
                                .description("Realm that character is on")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("recentplaytime")
                        .description("Shows video game playtime over a specified duration of a user or everyone in this server")
                        .create_option(|o| {
                            o.name("duration")
                                .description("Duration to show playtime for (1 week, 2 months, etc)")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("user")
                                .description("User to show playtime for")
                                .kind(CommandOptionType::User)
                                .required(false)
                        })
                })
                .create_application_command(|c| {
                    c.name("roll")
                        .description("Roll a die")
                        .create_option(|o| {
                            o.name("sides")
                                .description("Sides on die (default 100)")
                                .kind(CommandOptionType::Integer)
                                .required(false)
                                .min_int_value(1)
                                .max_int_value(u32::MAX)
                        })
                })
                .create_application_command(|c| {
                    c.name("source").description("Sends link to bot source code")
                })
                /*.create_application_command(|c| {
                    c.name("tarkov")
                        .description("Sends flea market and vendor info for item")
                        .create_option(|o| {
                            o.name("item")
                                .description("Tarkov item to search the flea market for")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })*/
                .create_application_command(|c| {
                    c.name("top")
                        .description("Lists members by number of sent messages")
                        .create_option(|o| {
                            o.name("count")
                                .description("The number of members to list (defaults to 5)")
                                .kind(CommandOptionType::Integer)
                                .required(false)
                                .min_int_value(1)
                                .max_int_value(100)
                        })
                })
                .create_application_command(|c| {
                    c.name("topcommand")
                        .description("Lists members by most command invocations")
                        .create_option(|o| {
                            o.name("command")
                                .description("Command to list invocations for")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("toplength")
                        .description("Lists members by average length of sent messages")
                        .create_option(|o| {
                            o.name("count")
                                .description("The number of members to list (defaults to 5)")
                                .kind(CommandOptionType::Integer)
                                .required(false)
                                .min_int_value(1)
                                .max_int_value(100)
                        })
                })
                .create_application_command(|c| {
                    c.name("track")
                        .description("Track shipment")
                        .create_option(|o| {
                            o.name("carrier")
                                .description("Shipping company")
                                .kind(CommandOptionType::String)
                                .required(true)
                                .add_string_choice("FedEx", "fedex")
                                .add_string_choice("UPS", "ups")
                                .add_string_choice("USPS", "usps")
                        })
                        .create_option(|o| {
                            o.name("number")
                                .description("Tracking number")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("comment")
                                .description("Optional comment descriping shipment, will be sent to channel upon package delivery")
                                .kind(CommandOptionType::String)
                                .required(false)
                        })
                })
                .create_application_command(|c| {
                    c.name("userinfo").description("Displays details about a user")
                        .create_option(|o| {
                            o.name("user")
                                .description("User to display")
                                .kind(CommandOptionType::User)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("weather")
                        .description("Sends weather conditions for an area")
                        .create_option(|o| {
                            o.name("location")
                                .description("Area to get weather for; can be city name, postal code, or decimal lat/long (default: Austin, TX)")
                                .kind(CommandOptionType::String)
                                .required(false)
                        })
                })
                .create_application_command(|c| {
                    c.name("whois")
                        .description("Lookup username by ID")
                        .create_option(|o| {
                            o.name("id")
                                .description("ID of user to find")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("wow")
                        .description("World of Warcraft commands")
                        .create_option(|o| {
                            o.name("character")
                                .description("WoW character details")
                                .kind(CommandOptionType::SubCommand)
                                .create_sub_option(|s| {
                                    s.name("character")
                                        .description("Character name")
                                        .kind(CommandOptionType::String)
                                        .required(true)
                                })
                                .create_sub_option(|s| {
                                    s.name("realm")
                                        .description("Character's realm")
                                        .kind(CommandOptionType::String)
                                        .required(true)
                                })
                        })
                        .create_option(|o| {
                            o.name("realm")
                                .description("Status of WoW realm")
                                .kind(CommandOptionType::SubCommand)
                                .create_sub_option(|s| {
                                    s.name("realm")
                                        .description("Realm name")
                                        .kind(CommandOptionType::String)
                                        .required(true)
                                })
                        })
                        .create_option(|o| {
                            o.name("search")
                                .description("Search all realms for WoW character by name")
                                .kind(CommandOptionType::SubCommand)
                                .create_sub_option(|s| {
                                    s.name("character")
                                        .description("Character name")
                                        .kind(CommandOptionType::String)
                                        .required(true)
                                })
                        })
                        .create_option(|o| {
                            o.name("transmog")
                                .description("Image of character from WoW armory")
                                .kind(CommandOptionType::SubCommand)
                                .create_sub_option(|s| {
                                    s.name("character")
                                        .description("Character name")
                                        .kind(CommandOptionType::String)
                                        .required(true)
                                })
                                .create_sub_option(|s| {
                                    s.name("realm")
                                        .description("Character's realm")
                                        .kind(CommandOptionType::String)
                                        .required(true)
                                })
                        })
                })
                .create_application_command(|c| {
                    c.name("zalgo")
                        .description("HE COMES")
                        .create_option(|o| {
                            o.name("message")
                                .description("HE COMES")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })
        })
        .await {
            Ok(commands) => info!(commands = ?commands.iter().map(|g| &g.name).collect::<Vec<&String>>(), "commands set"),
            Err(e) => error!(%e, "error setting commands"),
        }
    }

    async fn presence_update(&self, ctx: Context, update: Presence) {
        handle_presence(&ctx, &self.db, update.guild_id, update).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            if let Err(e) = command
                .create_interaction_response(&ctx.http, |response| {
                    response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
                })
                .await
            {
                error!(%e, "Unable to defer response to interaction");
                report_interaction_error(
                    &ctx,
                    format!("unable to defer response to interaction: `{}`", e),
                )
                .await;
                return;
            }
            self.record_command(&command).await;
            if let Err(e) = match command.data.name.as_str() {
                "affixes" => commands::affixes::affixes(&ctx, &command).await,
                "birdtime" => commands::time::time(&ctx, &command, "Europe/Oslo").await,
                "botinfo" => commands::botinfo::botinfo(&ctx, &command).await,
                "forecast" => commands::weather::forecast(&ctx, &command).await,
                "fortune" => commands::fortune::fortune(&ctx, &command).await,
                "invite" => commands::invite::invite(&ctx, &command).await,
                "karma" => commands::karma::karma(&ctx, &command).await,
                "lastseen" => commands::lastseen::lastseen(&ctx, &command).await,
                "lastplayed" => commands::lastplayed::lastplayed(&ctx, &command).await,
                "mirotime" => commands::time::time(&ctx, &command, "Europe/Helsinki").await,
                "nieltime" => commands::time::time(&ctx, &command, "Europe/Stockholm").await,
                "ping" => commands::ping::ping(&ctx, &command).await,
                "playtime" => commands::playtime::playtime(&ctx, &command).await,
                "raiderio" => commands::raiderio::raiderio(&ctx, &command).await,
                "realtime" => commands::time::time(&ctx, &command, "America/Chicago").await,
                "recentplaytime" => commands::playtime::recent_playtime(&ctx, &command).await,
                "roll" => commands::roll::roll(&ctx, &command).await,
                "sebbitime" => commands::time::time(&ctx, &command, "Europe/Copenhagen").await,
                "serverinfo" => commands::serverinfo::serverinfo(&ctx, &command).await,
                "source" => commands::source::source(&ctx, &command).await,
                "tarkov" => commands::tarkov::tarkov(&ctx, &command).await,
                "top" => commands::top::top(&ctx, &command).await,
                "topcommand" => commands::topcommand::topcommand(&ctx, &command).await,
                "toplength" => commands::toplength::toplength(&ctx, &command).await,
                "track" => commands::shipping::track(&ctx, &command).await,
                "userinfo" => commands::userinfo::userinfo(&ctx, &command).await,
                "weather" => commands::weather::weather(&ctx, &command).await,
                "whois" => commands::whois::whois(&ctx, &command).await,
                "zalgo" => commands::zalgo::zalgo(&ctx, &command).await,
                "wow" => {
                    if let Some(subcommand) = command.data.options.get(0) {
                        match subcommand.name.as_str() {
                            "character" => {
                                commands::wow::character(&ctx, &command, &subcommand.options).await
                            }
                            "realm" => {
                                commands::wow::realm(&ctx, &command, &subcommand.options).await
                            }
                            "search" => {
                                commands::wow::search(&ctx, &command, &subcommand.options).await
                            }
                            "transmog" => {
                                commands::wow::transmog(&ctx, &command, &subcommand.options).await
                            }
                            _ => Err("Unrecognized wow subcommand".into()),
                        }
                    } else {
                        Err("Missing wow subcommand".into())
                    }
                }
                _ => {
                    error!(command = command.data.name, "Missing command");
                    report_interaction_error(
                        &ctx,
                        format!("missing command: {}", command.data.name),
                    )
                    .await;
                    if let Err(e) = command
                        .edit_original_interaction_response(&ctx.http, |response| {
                            response.content("\u{26A0} `Unknown command`")
                        })
                        .await
                    {
                        error!(%e, "Unable to respond to interaction");
                        report_interaction_error(
                            &ctx,
                            format!("unable to respond to interaction: `{}`", e),
                        )
                        .await;
                    }
                    Ok(())
                }
            } {
                error!(%e, command = command.data.name, "Error running command");
                report_interaction_error(
                    &ctx,
                    format!("error running {}: `{}`", command.data.name, e),
                )
                .await;
                if let Err(resp_e) = command
                    .edit_original_interaction_response(&ctx.http, |response| {
                        response.content(format!("\u{26A0} `Error: {}`", e))
                    })
                    .await
                {
                    error!(e = %resp_e, "Unable to respond to interaction");
                    report_interaction_error(
                        &ctx,
                        format!("unable to respond to interaction: `{}`", resp_e),
                    )
                    .await;
                }
            }
        } else if let Interaction::MessageComponent(interaction) = interaction {
            let fields: Vec<&str> = interaction.data.custom_id.split(':').collect();
            let command = fields[0];
            if command != "playtime" {
                return;
            }
            let prev_next = fields[1];
            let button_id = match fields[2].parse::<i32>() {
                Ok(id) => id,
                Err(e) => {
                    error!(error = %e, "error parsing button_id from playtime interaction");
                    return;
                }
            };
            let row = {
                let data = ctx.data.read().await;
                #[allow(clippy::unwrap_used)]
                let db = data.get::<model::DB>().unwrap();
                #[allow(clippy::panic)]
                match sqlx::query!(r#"SELECT author_id, user_ids, username, start_date, end_date, start_offset FROM playtime_button WHERE id = $1"#, button_id).fetch_one(db).await {
                        Ok(row) => row,
                        Err(e) => {
                            error!(error = %e, "error getting playtime interaction buttons");
                            return;
                        }
                    }
            };

            let user_ids: Vec<i64> = row.user_ids;
            let username: Option<String> = row.username;
            let start_date: Option<DateTime<Utc>> = row.start_date;
            let end_date: DateTime<Utc> = row.end_date;
            let offset: i32 = {
                if prev_next == "prev" {
                    (row.start_offset - i32::from(commands::playtime::OFFSET_INC)).max(0)
                } else if prev_next == "next" {
                    row.start_offset + i32::from(commands::playtime::OFFSET_INC)
                } else {
                    return;
                }
            };

            #[allow(clippy::unwrap_used)] // offset isn't negative
            let new_content = match commands::playtime::gen_playtime_message(
                &ctx,
                &user_ids,
                &username,
                start_date,
                end_date,
                usize::try_from(offset).unwrap(),
            )
            .await
            {
                Ok(c) => c,
                Err(e) => {
                    error!(error = %e, "error generating new content for playtime interaction");
                    return;
                }
            };
            if let Some(flags) = interaction.message.flags {
                if flags.contains(MessageFlags::EPHEMERAL) {
                    return;
                }
            }
            let mut message = interaction.message.clone();
            if let Err(e) = message
                .edit(&ctx, |m| {
                    m.content(&new_content);
                    m.components(|c| {
                        commands::playtime::create_components(
                            c,
                            offset,
                            &new_content,
                            button_id,
                            true,
                        )
                    });
                    m
                })
                .await
            {
                error!(error = %e, "error updating playtime messge components");
                return;
            }

            if let Err(e) = interaction
                .create_interaction_response(&ctx, |r| {
                    r.kind(InteractionResponseType::UpdateMessage);
                    r
                })
                .await
            {
                error!(error = %e, "error creating playtime interaction response");
                return;
            }

            {
                let data = ctx.data.read().await;
                #[allow(clippy::unwrap_used)]
                let db = data.get::<model::DB>().unwrap();
                #[allow(clippy::panic)]
                if let Err(e) = sqlx::query!(
                    r#"UPDATE playtime_button SET start_offset = $2 WHERE id = $1"#,
                    button_id,
                    offset
                )
                .execute(db)
                .await
                {
                    error!(error = %e, "error updating playtime_button table after interaction");
                }
            }

            // leave buttons disabled for 5 seconds, then send the message again with buttons enabled
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            if let Err(e) = message
                .edit(&ctx, |m| {
                    m.components(|c| {
                        commands::playtime::create_components(
                            c,
                            offset,
                            &new_content,
                            button_id,
                            false,
                        )
                    });
                    m
                })
                .await
            {
                error!(error = %e, "error updating playtime messge components");
                return;
            }
        }
    }

    async fn guild_create(&self, ctx: Context, guild: Guild, is_new: bool) {
        if is_new {
            info!(
                id = guild.id.0,
                members = guild.member_count,
                name = guild.name,
                "joined guild"
            );
        }
        for (_, presence) in guild.presences {
            handle_presence(&ctx, &self.db, Some(guild.id), presence).await;
        }
    }

    async fn guild_delete(&self, _: Context, guild: UnavailableGuild, _: Option<Guild>) {
        warn!(id = guild.id.0, offline = guild.unavailable, "left guild");
    }

    async fn guild_member_removal(
        &self,
        ctx: Context,
        guild_id: GuildId,
        user: User,
        _: Option<Member>,
    ) {
        // Check to see if we can still see this user in other guilds, if not mark them as offline in DB
        let guild_lists = {
            let data = ctx.data.read().await;
            #[allow(clippy::unwrap_used)]
            data.get::<model::UserGuildList>().unwrap().clone()
        };
        let is_empty = {
            let mut guild_lists = guild_lists.write().await;
            match guild_lists.get_mut(&user.id) {
                Some(user_list) => {
                    // Remove this guild ID from user's list and check if list is now empty
                    user_list.remove(&guild_id);
                    user_list.is_empty()
                }
                None => true,
            }
        };
        if is_empty {
            let user_id = match i64::try_from(user.id.0) {
                Ok(u) => u,
                Err(e) => {
                    error!(%e, "unable to fit user id in i64");
                    return;
                }
            };
            if let Err(e) = sqlx::query(
                r#"INSERT INTO user_presence (user_id, status) VALUES ($1, 'offline'::online_status)"#,
            )
            .bind(user_id)
                .execute(&self.db)
                .await
            {
                error!(%e, "Error saving user_presence");
            }
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        {
            let author_id = match i64::try_from(msg.author.id.0) {
                Ok(a) => a,
                Err(e) => {
                    error!(%e, "unable to fit author id in i64");
                    return;
                }
            };
            let channel_id = match i64::try_from(msg.channel_id.0) {
                Ok(c) => c,
                Err(e) => {
                    error!(%e, "unable to fit channel id in i64");
                    return;
                }
            };
            let guild_id = if let Some(g) = msg.guild_id {
                match i64::try_from(g.0) {
                    Ok(g) => Some(g),
                    Err(e) => {
                        error!(%e, "unable to fit guild id in i64");
                        return;
                    }
                }
            } else {
                None
            };

            #[allow(clippy::panic)]
            if let Err(e) = sqlx::query!(
                r#"
INSERT INTO message(discord_id, author_id, channel_id, guild_id, content)
VALUES ($1, $2, $3, $4, $5)"#,
                Decimal::from(msg.id.0),
                author_id,
                channel_id,
                guild_id,
                msg.content
            )
            .execute(&self.db)
            .await
            {
                error!(%e, "error inserting message into db");
            }
        }
        if let Some(caps) = self.twitch_regex.captures(&msg.content) {
            if let Some(channel_match) = caps.get(2) {
                let channel_name = channel_match.as_str();
                let (access_token, client_id) = match twitch::get_access_token(&ctx).await {
                    Ok(a) => a,
                    Err(e) => {
                        error!(%e, "error getting twitch auth");
                        return;
                    }
                };
                match twitch::get_stream_info(&access_token, &client_id, channel_name).await {
                    Ok(s) => {
                        if let Some(stream) = s {
                            if let Err(e) = msg
                                .channel_id
                                .send_message(ctx, |m| {
                                    m.content(format!(
                                        "{} playing {}\n{}\n{} viewers",
                                        stream.user_name,
                                        stream.game_name,
                                        stream.title,
                                        stream.viewer_count.to_formatted_string(&Locale::en)
                                    ))
                                })
                                .await
                            {
                                error!(%e, "error sending twitch message");
                            }
                        }
                    }
                    Err(e) => error!(%e, "error getting twitch stream info"),
                }
            }
        }
    }

    async fn message_delete(
        &self,
        _ctx: Context,
        channel_id: ChannelId,
        message_id: MessageId,
        _guild_id: Option<GuildId>,
    ) {
        let channel_id = match i64::try_from(channel_id.0) {
            Ok(c) => c,
            Err(e) => {
                error!(%e, "unable to fit channel id in i64");
                return;
            }
        };
        #[allow(clippy::panic)]
        if let Err(e) = sqlx::query!(
            r#"DELETE FROM message WHERE channel_id = $1 AND discord_id = $2"#,
            channel_id,
            Decimal::from(message_id.0)
        )
        .execute(&self.db)
        .await
        {
            error!(%e, "error deleting message from db");
        }
    }

    async fn message_delete_bulk(
        &self,
        _ctx: Context,
        channel_id: ChannelId,
        message_ids: Vec<MessageId>,
        _guild_id: Option<GuildId>,
    ) {
        let channel_id = match i64::try_from(channel_id.0) {
            Ok(c) => c,
            Err(e) => {
                error!(%e, "unable to fit channel id in i64");
                return;
            }
        };
        let decimal_message_ids: Vec<Decimal> = message_ids
            .into_iter()
            .map(|m| Decimal::from(m.0))
            .collect();
        #[allow(clippy::panic)]
        if let Err(e) = sqlx::query!(
            r#"DELETE FROM message WHERE channel_id = $1 AND discord_id = ANY($2)"#,
            channel_id,
            &decimal_message_ids
        )
        .execute(&self.db)
        .await
        {
            error!(%e, "error bulk deleting messages from db");
        }
    }

    async fn message_update(
        &self,
        _ctx: Context,
        _old: Option<Message>,
        _new: Option<Message>,
        update: MessageUpdateEvent,
    ) {
        let content: String = if let Some(c) = update.content {
            c
        } else {
            return;
        };
        let channel_id = match i64::try_from(update.channel_id.0) {
            Ok(c) => c,
            Err(e) => {
                error!(%e, "unable to fit channel id in i64");
                return;
            }
        };
        #[allow(clippy::panic)]
        if let Err(e) = sqlx::query!(
            r#"UPDATE message SET content = $1 WHERE channel_id = $2 AND discord_id = $3"#,
            content,
            channel_id,
            Decimal::from(update.id.0)
        )
        .execute(&self.db)
        .await
        {
            error!(%e, "error editing message in db");
        }
    }
}

async fn handle_presence(
    ctx: &Context,
    db: &Pool<Postgres>,
    guild_id: Option<GuildId>,
    presence: Presence,
) {
    let user_id = presence.user.id;
    if match presence.user.bot {
        Some(bot) => bot,
        None => match ctx.cache.user(user_id) {
            Some(user) => user.bot,
            None => {
                if let Ok(user) = ctx.http.get_user(user_id.0).await {
                    user.bot
                } else {
                    warn!(user_id = user_id.0, "Unable to determine if user is bot");
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
            if game_name.starts_with(char::is_whitespace)
                || game_name.ends_with(char::is_whitespace)
            {
                game_name = game_name.trim().to_owned();
            }
            Some(game_name)
        } else {
            None
        }
    });

    if guild_id.is_none() {
        warn!(user_id = user_id.0, status = ?presence.status, ?game_name, "Presence without guild");
    }

    // Check if we've already recorded that user is in this guild
    if let Some(guild_id) = guild_id {
        let guild_lists = {
            let data = ctx.data.read().await;
            #[allow(clippy::unwrap_used)]
            data.get::<model::UserGuildList>().unwrap().clone()
        };
        let in_guild_list = {
            let in_guild_list = match guild_lists.read().await.get(&user_id) {
                Some(g) => g.contains(&guild_id),
                None => false,
            };
            in_guild_list
        };
        // Add guild ID to user's list, creating list for user if they're new
        if !in_guild_list {
            let mut guild_lists = guild_lists.write().await;
            if let Some(l) = guild_lists.get_mut(&user_id) {
                l.insert(guild_id);
            } else {
                let mut new_list = HashSet::new();
                new_list.insert(guild_id);
                guild_lists.insert(user_id, new_list);
            }
        }
    }

    // Do nothing if presence's status and game name haven't changed since the last update we saw
    let last_presence_map = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        if let Some(last_presence) = data
            .get::<model::LastUserPresence>()
            .unwrap()
            .clone()
            .read()
            .await
            .get(&user_id)
        {
            if last_presence.status == presence.status && last_presence.game_name == game_name {
                return;
            }
        }

        let user_id = match i64::try_from(user_id.0) {
            Ok(u) => u,
            Err(e) => {
                error!(%e, "unable to fit user id in i64");
                return;
            }
        };

        if let Err(e) = sqlx::query(
            r#"INSERT INTO user_presence (user_id, status, game_name) VALUES ($1, $2::online_status, $3)"#,
        )
        .bind(user_id)
            .bind(presence.status.name())
            .bind(&game_name)
            .execute(db)
            .await
        {
            error!(%e, "Error saving user_presence");
            return;
        }

        #[allow(clippy::unwrap_used)]
        data.get::<model::LastUserPresence>().unwrap().clone()
    };
    let mut last_presence_map = last_presence_map.write().await;
    last_presence_map.insert(
        user_id,
        model::UserPresence {
            status: presence.status,
            game_name,
        },
    );
}

async fn report_interaction_error(ctx: &Context, error: String) {
    let owner_id = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        UserId::from(*data.get::<model::OwnerId>().unwrap())
    };
    let channel = match owner_id.create_dm_channel(&ctx.http).await {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "error creating owner DM channel");
            return;
        }
    };
    if let Err(e) = channel.say(&ctx.http, error).await {
        error!(error = %e, "error messaging owner DM channel");
    }
}
