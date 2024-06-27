use crate::error::CommandResult;
use crate::model::DB;
use crate::util;
use chrono::prelude::*;
use serenity::all::{CommandDataOptionValue, CommandInteraction};
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;
use serenity::model::user::OnlineStatus;

#[allow(clippy::similar_names)]
// Replies to msg with the duration since the user was last online
pub async fn lastseen(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let Some(user_id) = interaction.data.options.first().and_then(|o| {
        if let CommandDataOptionValue::User(u) = o.value {
            Some(u)
        } else {
            None
        }
    }) else {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Unable to find user"),
            )
            .await?;
        return Ok(());
    };

    let Some(guild_id) = interaction.guild_id else {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Command can only be used in a server"),
            )
            .await?;
        return Ok(());
    };

    let username = if let Ok(user) = user_id.to_user(ctx).await {
        user.nick_in(ctx, guild_id).await.or(Some(user.name))
    } else {
        None
    };

    if let Some(status) = util::get_user_status(ctx, guild_id, user_id).await {
        use OnlineStatus::*;
        match status {
            Online | DoNotDisturb => {
                let content = if let Some(username) = username {
                    format!("{username} is currently online")
                } else {
                    "They're currently online".to_string()
                };
                interaction
                    .edit_response(&ctx.http, EditInteractionResponse::new().content(content))
                    .await?;
                return Ok(());
            }
            Idle | Invisible | Offline => {}
            _ => return Err(format!("unrecognized OnlineStatus: {status:?}").into()),
        }
    }

    let Some(row) = ({
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        let db = data.get::<DB>().unwrap();
        #[allow(clippy::panic)]
        sqlx::query!(r"SELECT create_date FROM user_presence WHERE user_id = $1 AND (status = 'offline' OR status = 'invisible' OR status = 'idle') ORDER BY create_date DESC LIMIT 1", i64::from(user_id)).fetch_optional(db).await?
    }) else {
        let content = if let Some(username) = username {
            format!("I've never seen {username}")
        } else {
            "I've never seen them".to_string()
        };
        interaction
            .edit_response(&ctx.http, EditInteractionResponse::new().content(content))
            .await?;
        return Ok(());
    };

    let now = Local::now().with_timezone(Local::now().offset());
    let last_seen = row.create_date;
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

    let content = if let Some(username) = username {
        format!("{username} was last seen {since_str} ago")
    } else {
        format!("last seen {since_str} ago")
    };
    interaction
        .edit_response(&ctx.http, EditInteractionResponse::new().content(content))
        .await?;

    Ok(())
}
