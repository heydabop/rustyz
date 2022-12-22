mod interaction;
mod message;
mod presence;

use crate::model;

use serde_json::json;
use serenity::async_trait;
use serenity::client::{Context, EventHandler};
use serenity::json::Value;
use serenity::model::{
    application::command::{Command, CommandOptionType},
    application::interaction::{application_command::ApplicationCommandInteraction, Interaction},
    channel::Message,
    event::MessageUpdateEvent,
    gateway::{Presence, Ready},
    guild::{Guild, Member, UnavailableGuild},
    id::{ChannelId, GuildId, MessageId, UserId},
    user::User,
};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
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
        presence::update(&ctx, &self.db, update.guild_id, update).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        interaction::create(ctx, &self.db, interaction).await;
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
            presence::update(&ctx, &self.db, Some(guild.id), presence).await;
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
        message::create(ctx, &self.db, &self.twitch_regex, msg).await;
    }

    async fn message_delete(
        &self,
        _ctx: Context,
        channel_id: ChannelId,
        message_id: MessageId,
        _guild_id: Option<GuildId>,
    ) {
        message::delete(&self.db, channel_id, message_id).await;
    }

    async fn message_delete_bulk(
        &self,
        _ctx: Context,
        channel_id: ChannelId,
        message_ids: Vec<MessageId>,
        _guild_id: Option<GuildId>,
    ) {
        message::delete_bulk(&self.db, channel_id, message_ids).await;
    }

    async fn message_update(
        &self,
        _ctx: Context,
        _old: Option<Message>,
        _new: Option<Message>,
        update: MessageUpdateEvent,
    ) {
        message::update(&self.db, update).await;
    }
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

async fn record_command(db: &Pool<Postgres>, command: &ApplicationCommandInteraction) {
    let mut command_name = command.data.name.clone();
    let log_options: HashMap<&String, &Option<Value>> =
        if let Some(option) = command.data.options.get(0) {
            if option.kind == CommandOptionType::SubCommand {
                command_name = format!("{command_name} {}", option.name);
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
    .execute(db)
    .await
    {
        error!(%e, "error inserting command log into db");
    }
}
