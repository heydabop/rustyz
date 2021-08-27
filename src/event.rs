use crate::commands::playtime::gen_playtime_message;
use crate::model;

use chrono::prelude::*;
use serenity::async_trait;
use serenity::client::{Context, EventHandler};
use serenity::model::{
    event::PresenceUpdateEvent,
    gateway::{ActivityType, Ready},
    interactions::{
        message_component::{ButtonStyle, InteractionMessage},
        Interaction, InteractionApplicationCommandCallbackDataFlags, InteractionResponseType,
    },
};
use sqlx::Row;

pub struct Handler;

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
            if let Some(last_presence) =
                data.get::<model::LastUserPresence>().unwrap().get(&user_id)
            {
                if last_presence.status == presence.status && last_presence.game_name == game_name {
                    return;
                }
            }
        }

        let mut data = ctx.data.write().await;
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
        let last_presence_map = data.get_mut::<model::LastUserPresence>().unwrap();
        last_presence_map.insert(
            user_id,
            model::UserPresence {
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
            offset = (offset - 10).max(0);
        } else if prev_next == "next" {
            offset += 10;
        } else {
            return;
        }

        #[allow(clippy::cast_sign_loss)]
        let new_content =
            gen_playtime_message(&ctx, &user_ids, &username, start_date, end_date, offset as usize)
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
}
