use crate::error::CommandResult;
use crate::model::{DB, LastUserPresence};
use chrono::prelude::*;
use serenity::all::{CommandDataOptionValue, CommandInteraction};
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;
use serenity::model::user::OnlineStatus;

#[allow(clippy::similar_names)]
pub async fn lastplayed(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
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

    let last_presence = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<LastUserPresence>().unwrap().clone()
    };

    if let Some(presence) = last_presence.read().await.get(&user_id)
        && presence.status != OnlineStatus::Offline
        && presence.status != OnlineStatus::Invisible
        && let Some(game_name) = &presence.game_name
    {
        let content = if let Some(username) = username {
            format!("{username} is currently playing {game_name}")
        } else {
            format!("currently playing {game_name}")
        };
        interaction
            .edit_response(&ctx.http, EditInteractionResponse::new().content(content))
            .await?;
        return Ok(());
    }

    let db = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<DB>().unwrap().clone()
    };
    #[allow(clippy::panic)]
    let Some(row) = sqlx::query!(r"SELECT create_date, game_name FROM user_presence WHERE user_id = $1 AND status <> 'offline' AND status <> 'invisible' AND game_name IS NOT NULL ORDER BY create_date DESC LIMIT 1", i64::from(user_id)).fetch_optional(&db).await? else {
        interaction
            .edit_response(&ctx.http, EditInteractionResponse::new()
                           .content(
                               format!("I've never seen {} play anything",
                                       if let Some(username) = username {
                                           username
                                       } else {
                                           String::from("them")
                                       }
                               )
                           )
            )
            .await?;
        return Ok(());
    };
    let start = row.create_date;
    let game_name = row.game_name.unwrap_or_default();
    // get row without game_name inserted after the game row to determine when user stopped playing
    #[allow(clippy::panic)]
    let Some(end_row) = sqlx::query!(r"SELECT create_date FROM user_presence WHERE user_id = $1 AND game_name IS NULL AND create_date > $2 ORDER BY create_date ASC LIMIT 1", i64::from(user_id), start).fetch_optional(&db).await? else {
        let content = if let Some(username) = username {
            format!("{username} is currently playing {game_name}")
        } else {
            format!("currently playing {game_name}")
        };
        interaction
            .edit_response(&ctx.http, EditInteractionResponse::new().content(content))
            .await?;
        return Ok(());
    };
    drop(db);
    let stopped_playing = end_row.create_date;

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

    let content = if let Some(username) = username {
        format!("{username} was playing {game_name} {since_str} ago",)
    } else {
        format!("was playing {game_name} {since_str} ago")
    };

    interaction
        .edit_response(&ctx.http, EditInteractionResponse::new().content(content))
        .await?;

    Ok(())
}
