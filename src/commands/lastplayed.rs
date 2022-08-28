use crate::model::{LastUserPresence, DB};
use chrono::prelude::*;
use serenity::client::Context;
use serenity::framework::standard::CommandResult;
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
    let user = match interaction.data.options.get(0).and_then(|o| {
        o.resolved.as_ref().and_then(|r| {
            if let CommandDataOptionValue::User(u, _) = r {
                Some(u)
            } else {
                None
            }
        })
    }) {
        Some(u) => u,
        None => {
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content("Unable to find user")
                })
                .await?;
            return Ok(());
        }
    };

    let last_presence = {
        let data = ctx.data.read().await;
        data.get::<LastUserPresence>().unwrap().clone()
    };

    if let Some(presence) = last_presence.read().await.get(&user.id) {
        if presence.status != OnlineStatus::Offline && presence.status != OnlineStatus::Invisible {
            if let Some(game_name) = &presence.game_name {
                interaction
                    .edit_original_interaction_response(&ctx.http, |response| {
                        response
                            .content(format!("{} is currently playing {}", user.name, game_name))
                    })
                    .await?;
                return Ok(());
            }
        }
    }

    #[allow(clippy::cast_possible_wrap)]
    let row = {
        let data = ctx.data.read().await;
        let db = data.get::<DB>().unwrap();
        sqlx::query(r#"SELECT create_date, game_name FROM user_presence WHERE user_id = $1 AND status <> 'offline' AND status <> 'invisible' AND game_name IS NOT NULL ORDER BY create_date DESC LIMIT 1"#).bind(i64::from(user.id)).fetch_optional(db).await?
    };
    if row.is_none() {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content(format!("I've never seen {} play anything", user.name))
            })
            .await?;
        return Ok(());
    }
    let row = row.unwrap();

    let now = Local::now().with_timezone(Local::now().offset());
    let last_playing = row.get::<DateTime<FixedOffset>, _>(0);
    let game_name = row.get::<String, _>(1);
    let since = now.signed_duration_since(last_playing);

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
                "{} was playing {} {} ago",
                user.name, game_name, since_str
            ))
        })
        .await?;

    Ok(())
}
