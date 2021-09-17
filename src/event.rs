use crate::commands::playtime::{create_components, gen_playtime_message};
use crate::model;

use chrono::prelude::*;
use serenity::async_trait;
use serenity::client::{Context, EventHandler};
use serenity::model::{
    event::PresenceUpdateEvent,
    gateway::{ActivityType, Presence, Ready},
    guild::{Guild, Member},
    id::GuildId,
    interactions::{
        message_component::InteractionMessage, Interaction,
        InteractionApplicationCommandCallbackDataFlags, InteractionResponseType,
    },
    user::User,
};
use sqlx::Row;
use std::collections::HashSet;

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Bot {} is successfully connected.", ready.user.name);
    }

    async fn presence_update(&self, ctx: Context, update: PresenceUpdateEvent) {
        handle_presence(&ctx, update.guild_id, update.presence).await;
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
        let button_id = fields[2].parse::<i32>().unwrap();
        let row = {
            let data = ctx.data.read().await;
            let db = data.get::<model::DB>().unwrap();
            match sqlx::query(r#"SELECT author_id, user_ids, username, start_date, end_date, start_offset FROM playtime_button WHERE id = $1"#).bind(button_id).fetch_one(&*db).await {
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
        let start_date = row.get::<Option<DateTime<FixedOffset>>, _>(3);
        let end_date = row.get::<DateTime<FixedOffset>, _>(4);
        let mut offset = row.get::<i32, _>(5);

        if prev_next == "prev" {
            offset = (offset - 15).max(0);
        } else if prev_next == "next" {
            offset += 15;
        } else {
            return;
        }

        #[allow(clippy::cast_sign_loss)]
        let new_content = gen_playtime_message(
            &ctx,
            &user_ids,
            &username,
            start_date,
            end_date,
            offset as usize,
        )
        .await
        .unwrap();
        let mut message = match interaction.message {
            InteractionMessage::Regular(ref m) => m.clone(),
            InteractionMessage::Ephemeral(_) => return,
        };
        if let Err(e) = message
            .edit(&ctx, |m| {
                m.content(&new_content);
                m.components(|c| create_components(c, offset, &new_content, button_id));
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
            let db = data.get::<model::DB>().unwrap();
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