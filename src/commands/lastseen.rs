use crate::model::DB;
use crate::util;
use chrono::prelude::*;
use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::user::OnlineStatus;
use sqlx::Row;

#[allow(clippy::similar_names)]
// Replies to msg with the duration since the user was last online
pub async fn lastseen(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
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

    if let Some(status) = util::get_user_status(ctx, interaction.guild_id.unwrap(), user.id).await {
        if status != OnlineStatus::Offline && status != OnlineStatus::Invisible {
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content(format!("{} is currently online", user.name))
                })
                .await?;
            return Ok(());
        }
    }

    #[allow(clippy::cast_possible_wrap)]
    let row = {
        let data = ctx.data.read().await;
        let db = data.get::<DB>().unwrap();
        sqlx::query(r#"SELECT create_date FROM user_presence WHERE user_id = $1 AND (status = 'offline' OR status = 'invisible') ORDER BY create_date DESC LIMIT 1"#).bind(i64::from(user.id)).fetch_optional(db).await?
    };
    if row.is_none() {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content(format!("I've never seen {}", user.name))
            })
            .await?;
        return Ok(());
    }

    let now = Local::now().with_timezone(Local::now().offset());
    let last_seen = row.unwrap().get::<DateTime<FixedOffset>, _>(0);
    let since = now.signed_duration_since(last_seen);

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
            response.content(format!("{} was last seen {} ago", user.name, since_str))
        })
        .await?;

    Ok(())
}
