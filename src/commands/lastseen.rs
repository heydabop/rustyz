use crate::model::DB;
use crate::util;
use chrono::prelude::*;
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::{channel::Message, user::OnlineStatus};
use sqlx::Row;

// Replies to msg with the duration since the user was last online
#[command]
#[only_in(guilds)]
pub async fn lastseen(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let user = match util::user_from_mention(ctx, msg, args.rest()).await? {
        Some(u) => u,
        None => return Ok(()),
    };

    if let Some(status) = user.status {
        if status != OnlineStatus::Offline && status != OnlineStatus::Invisible {
            util::record_say(ctx, msg, format!("{} is currently online", user.name)).await?;
            return Ok(());
        }
    }

    #[allow(clippy::cast_possible_wrap)]
    let row = {
        let data = ctx.data.read().await;
        let db = data.get::<DB>().unwrap();
        sqlx::query(r#"SELECT create_date FROM user_presence WHERE user_id = $1 AND (status = 'offline' OR status = 'invisible') ORDER BY create_date DESC LIMIT 1"#).bind(user.id as i64).fetch_optional(&*db).await?
    };
    if row.is_none() {
        util::record_say(ctx, msg, format!("I've never seen {}", user.name)).await?;
        return Ok(());
    }

    let now = Local::now();
    let now = now.with_timezone(now.offset());
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

    util::record_say(
        ctx,
        msg,
        format!("{} was last seen {} ago", user.name, since_str),
    )
    .await?;

    Ok(())
}
