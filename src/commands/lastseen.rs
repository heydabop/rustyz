use crate::error::CommandResult;
use crate::model::DB;
use crate::util;
use chrono::prelude::*;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::user::OnlineStatus;
use sqlx::Row;

#[allow(clippy::similar_names)]
// Replies to msg with the duration since the user was last online
pub async fn lastseen(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
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

    let Some(guild_id) = interaction.guild_id else {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content("Command can only be used in a server")
            })
            .await?;
        return Ok(());
    };

    if let Some(status) = util::get_user_status(ctx, guild_id, user.id).await {
        if status != OnlineStatus::Offline && status != OnlineStatus::Invisible {
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content(format!("{} is currently online", user.name))
                })
                .await?;
            return Ok(());
        }
    }

    let Some(row) = ({
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        let db = data.get::<DB>().unwrap();
        sqlx::query(r"SELECT create_date FROM user_presence WHERE user_id = $1 AND (status = 'offline' OR status = 'invisible') ORDER BY create_date DESC LIMIT 1").bind(i64::try_from(user.id)?).fetch_optional(db).await?
    }) else {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content(format!("I've never seen {}", user.name))
            })
            .await?;
        return Ok(());
    };

    let now = Local::now().with_timezone(Local::now().offset());
    let last_seen = row.get::<DateTime<FixedOffset>, _>(0);
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
            response.content(format!("{} was last seen {since_str} ago", user.name))
        })
        .await?;

    Ok(())
}
