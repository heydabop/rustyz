use crate::commands;
use crate::model;

use serenity::async_trait;
use serenity::client::{Context, EventHandler};
use serenity::model::{
    gateway::{ActivityType, Presence, Ready},
    guild::{Guild, Member},
    id::GuildId,
    interactions::{
        application_command::{ApplicationCommand, ApplicationCommandOptionType},
        Interaction,
    },
    user::User,
};
use std::collections::HashSet;

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("Bot {} is successfully connected.", ready.user.name);

        #[allow(clippy::unreadable_literal)]
        let g = GuildId(184428741450006528);
        match g.set_application_commands(&ctx.http, |commands| {
            commands.
                create_application_command(|c| {
                    c.name("birdtime").description("Sends the current time for bird")
                }).
                create_application_command(|c| {
                    c.name("mirotime").description("Sends the current time for miro")
                }).
                create_application_command(|c| {
                    c.name("nieltime").description("Sends the current time for niel")
                }).
                create_application_command(|c| {
                    c.name("realtime").description("Sends the current time for the mainlanders")
                }).
                create_application_command(|c| {
                    c.name("sebbitime").description("Sends the current time for sebbi")
                })
        })
        .await {
            Ok(guild_commands) => println!("guild commands set: {:?}", guild_commands.iter().map(|g| &g.name).collect::<Vec<&String>>()),
            Err(e) => eprintln!("error setting guild commands: {}", e),
        }

        match ApplicationCommand::set_global_application_commands(&ctx.http, |commands| {
            commands
                .create_application_command(|c| {
                    c.name("affixes").description("Sends this week's US Mythic+ affixes")
                })
                .create_application_command(|c| {
                    c.name("fortune").description("Sends a random adage")
                })
                .create_application_command(|c| {
                    c.name("karma")
                        .description("Lists members by karma points")
                        .create_option(|o| {
                            o.name("count")
                                .description("The number of members to list (defaults to 5)")
                                .kind(ApplicationCommandOptionType::Integer)
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
                                .kind(ApplicationCommandOptionType::User)
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
                                .kind(ApplicationCommandOptionType::User)
                                .required(false)
                        })
                })
                .create_application_command(|c| {
                    c.name("raiderio")
                        .description("Displays raider.io stats for given character")
                        .create_option(|o| {
                            o.name("character")
                                .description("Character to get stats for")
                                .kind(ApplicationCommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("realm")
                                .description("Realm that character is on")
                                .kind(ApplicationCommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("recentplaytime")
                        .description("Shows video game playtime over a specified duration of a user or everyone in this server")
                        .create_option(|o| {
                            o.name("duration")
                                .description("Duration to show playtime for (1 week, 2 months, etc)")
                                .kind(ApplicationCommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("user")
                                .description("User to show playtime for")
                                .kind(ApplicationCommandOptionType::User)
                                .required(false)
                        })
                })
                .create_application_command(|c| {
                    c.name("roll")
                        .description("Roll a die")
                        .create_option(|o| {
                            o.name("sides")
                                .description("Sides on die (default 100)")
                                .kind(ApplicationCommandOptionType::Integer)
                                .required(false)
                                .min_int_value(1)
                        })
                })
                .create_application_command(|c| {
                    c.name("source").description("Sends link to bot source code")
                })
                .create_application_command(|c| {
                    c.name("tarkov")
                        .description("Sends flea market and vendor info for item")
                        .create_option(|o| {
                            o.name("item")
                                .description("Tarkov item to search the flea market for")
                                .kind(ApplicationCommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("top")
                        .description("Lists members by number of sent messages")
                        .create_option(|o| {
                            o.name("count")
                                .description("The number of members to list (defaults to 5)")
                                .kind(ApplicationCommandOptionType::Integer)
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
                                .kind(ApplicationCommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|c| {
                    c.name("toplength")
                        .description("Lists members by average length of sent messages")
                        .create_option(|o| {
                            o.name("count")
                                .description("The number of members to list (defaults to 5)")
                                .kind(ApplicationCommandOptionType::Integer)
                                .required(false)
                                .min_int_value(1)
                                .max_int_value(100)
                        })
                })
                .create_application_command(|c| {
                    c.name("weather")
                        .description("Sends weather conditions for an area")
                        .create_option(|o| {
                            o.name("location")
                                .description("Area to get weather for; can be city name, postal code, or decimal lat/long (default: Austin, TX)")
                                .kind(ApplicationCommandOptionType::String)
                                .required(false)
                        })
                })
                .create_application_command(|c| {
                    c.name("whois")
                        .description("Lookup username by ID")
                        .create_option(|o| {
                            o.name("id")
                                .description("ID of user to find")
                                .kind(ApplicationCommandOptionType::String)
                                .required(false)
                        })
                })
        })
        .await {
            Ok(commands) => println!("commands set: {:?}", commands.iter().map(|g| &g.name).collect::<Vec<&String>>()),
            Err(e) => eprintln!("error setting commands: {}", e),
        }
    }

    async fn presence_update(&self, ctx: Context, update: Presence) {
        handle_presence(&ctx, update.guild_id, update).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            if let Err(e) = match command.data.name.as_str() {
                "affixes" => commands::affixes::affixes(&ctx, &command).await,
                "birdtime" => commands::time::time(&ctx, &command, "Europe/Oslo").await,
                "fortune" => commands::fortune::fortune(&ctx, &command).await,
                "karma" => commands::karma::karma(&ctx, &command).await,
                "lastseen" => commands::lastseen::lastseen(&ctx, &command).await,
                "mirotime" => commands::time::time(&ctx, &command, "Europe/Helsinki").await,
                "nieltime" => commands::time::time(&ctx, &command, "Europe/Stockholm").await,
                "ping" => commands::ping::ping(&ctx, &command).await,
                "playtime" => commands::playtime::playtime(&ctx, &command).await,
                "raiderio" => commands::raiderio::raiderio(&ctx, &command).await,
                "realtime" => commands::time::time(&ctx, &command, "America/Chicago").await,
                "recentplaytime" => commands::playtime::recent_playtime(&ctx, &command).await,
                "roll" => commands::roll::roll(&ctx, &command).await,
                "sebbitime" => commands::time::time(&ctx, &command, "Europe/Copenhagen").await,
                "source" => commands::source::source(&ctx, &command).await,
                "tarkov" => commands::tarkov::tarkov(&ctx, &command).await,
                "top" => commands::top::top(&ctx, &command).await,
                "topcommand" => commands::topcommand::topcommand(&ctx, &command).await,
                "toplength" => commands::toplength::toplength(&ctx, &command).await,
                "weather" => commands::weather::weather(&ctx, &command).await,
                "whois" => commands::whois::whois(&ctx, &command).await,
                _ => Ok(()),
            } {
                println!("Cannot respond to slash command: {}", e);
            }
        }
    }

    async fn guild_create(&self, ctx: Context, guild: Guild, _: bool) {
        for (_, presence) in guild.presences {
            handle_presence(&ctx, Some(guild.id), presence).await;
        }
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
            let data = ctx.data.read().await;
            let db = data.get::<model::DB>().unwrap();
            #[allow(clippy::cast_possible_wrap)] if let Err(e) = sqlx::query(
                r#"INSERT INTO user_presence (user_id, status) VALUES ($1, 'offline'::online_status)"#,
            )
            .bind(user.id.0 as i64)
                .execute(&*db)
                .await
            {
                println!("Error saving user_presence: {}", e);
            }
        }
    }
}

async fn handle_presence(ctx: &Context, guild_id: Option<GuildId>, presence: Presence) {
    let user_id = presence.user.id;
    if match presence.user.bot {
        Some(bot) => bot,
        None => match ctx.cache.user(user_id) {
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
        println!(
            "Presence without guild: {} {:?} {:?}",
            user_id, presence.status, game_name
        );
    }

    // Check if we've already recorded that user is in this guild
    if let Some(guild_id) = guild_id {
        let guild_lists = {
            let data = ctx.data.read().await;
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

        let db = data.get::<model::DB>().unwrap();
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
