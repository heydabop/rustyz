use crate::error::CommandResult;
use crate::model::{LastUserPresence, DB};
use chrono::prelude::*;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::user::OnlineStatus;
use sqlx::Row;

#[allow(clippy::similar_names)]
pub async fn lastplayed(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
) -> CommandResult {
    let Some(user) = interaction.data.options.get(0).and_then(|o| {
        o.resolved.as_ref().and_then(|r| {
            if let CommandDataOptionValue::User(u, _) = r {
                Some(u)
            } else {
                None
            }
        })
    }) else {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content("Unable to find user")
            })
            .await?;
        return Ok(());
    };

    let last_presence = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<LastUserPresence>().unwrap().clone()
    };

    if let Some(presence) = last_presence.read().await.get(&user.id) {
        if presence.status != OnlineStatus::Offline && presence.status != OnlineStatus::Invisible {
            if let Some(game_name) = &presence.game_name {
                interaction
                    .edit_original_interaction_response(&ctx.http, |response| {
                        response.content(format!("{} is currently playing {game_name}", user.name))
                    })
                    .await?;
                return Ok(());
            }
        }
    }

    let db = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<DB>().unwrap().clone()
    };
    let Some(row) = sqlx::query(r#"SELECT create_date, game_name FROM user_presence WHERE user_id = $1 AND status <> 'offline' AND status <> 'invisible' AND game_name IS NOT NULL ORDER BY create_date DESC LIMIT 1"#).bind(i64::try_from(user.id)?).fetch_optional(&db).await? else {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content(format!("I've never seen {} play anything", user.name))
            })
            .await?;
        return Ok(());
    };
    let start = row.get::<DateTime<FixedOffset>, _>(0);
    let game_name = row.get::<String, _>(1);
    // get row without game_name inserted after the game row to determine when user stopped playing
    let Some(end_row) = sqlx::query(r#"SELECT create_date FROM user_presence WHERE user_id = $1 AND game_name IS NULL AND create_date > $2 ORDER BY create_date ASC LIMIT 1"#).bind(i64::try_from(user.id)?).bind(start).fetch_optional(&db).await? else {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content(format!("{} is currently playing {game_name}", user.name))
            })
            .await?;
        return Ok(());
    };
    drop(db);
    let stopped_playing = end_row.get::<DateTime<FixedOffset>, _>(0);

    let now = Local::now().with_timezone(Local::now().offset());
    let since = now.signed_duration_since(stopped_playing);

    let since_str = if since.num_seconds() < 1 {
        String::from("less than a second")
    } else if since.num_seconds() < 120 {
        format!("{} seconds", since.num_seconds())
    } else if since.num_minutes() < 120 {
        format!("{} minutes", since.num_minutes())
    } else if since.num_hours() < 48 {
        format!("{} hours", since.num_hours())
    } else {
        format!("{} days", since.num_days())
    };

    interaction
        .edit_original_interaction_response(&ctx.http, |response| {
            response.content(format!(
                "{} was playing {game_name} {since_str} ago",
                user.name
            ))
        })
        .await?;

    Ok(())
}
