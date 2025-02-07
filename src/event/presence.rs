use crate::model;
use serenity::client::Context;
use serenity::model::{
    gateway::{ActivityType, Presence},
    id::GuildId,
};
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use tracing::{error, warn};

pub async fn update(
    ctx: &Context,
    db: &Pool<Postgres>,
    guild_id: Option<GuildId>,
    presence: Presence,
    is_startup: bool,
) {
    let user_id = presence.user.id;
    if match presence.user.bot {
        Some(bot) => bot,
        None => {
            let cache_user_bot = {
                // put non-send in block to clarify to rustc that its dropped before .await
                // this patterns comes up in a few places in commands
                if let Some(user) = ctx.cache.user(user_id) {
                    Some(user.bot)
                } else {
                    None
                }
            };
            match cache_user_bot {
                Some(bot) => bot,
                None => {
                    if let Ok(user) = ctx.http.get_user(user_id).await {
                        user.bot
                    } else {
                        warn!(
                            user_id = user_id.get(),
                            "Unable to determine if user is bot"
                        );
                        false
                    }
                }
            }
        }
    } {
        // ignore updates from bots
        return;
    }
    let game_name = presence.activities.iter().find_map(|a| {
        if a.kind == ActivityType::Playing {
            // clients reporting ® and ™ seems inconsistent, so the same game gets different names over time
            let mut game_name = a.name.replace(&['®', '™'][..], "");
            game_name.truncate(512);
            #[allow(clippy::assigning_clones)]
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
        warn!(user_id = user_id.get(), status = ?presence.status, ?game_name, "Presence without guild");
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

        #[allow(clippy::panic)]
        if let Err(e) = sqlx::query!(
            r"INSERT INTO user_presence (user_id, status, game_name, is_startup) VALUES ($1, $2::online_status, $3, $4)",
            i64::from(user_id),
            presence.status.name() as _,
            game_name,
            is_startup
        )
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
