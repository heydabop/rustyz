use crate::util;
use crate::OldDB;
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandError, CommandResult};
use serenity::model::channel::Message;
use sqlx::Row;

// Replies with the top users in guild sorted by highest karma (vote count)
// Allows a single optional arg of how many users to list, defaults to 5
#[command]
#[only_in(guilds)]
pub async fn karma(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild_id = if let Some(guild_id) = msg.guild_id {
        guild_id.0
    } else {
        return Err(CommandError::from("Unable to get guild ID of message"));
    };
    let usernames = util::collect_usernames(ctx, msg).await;
    let limit: u32 = args.single().unwrap_or(5).min(100);

    let rows = {
        let data = ctx.data.read().await;
        let db = data.get::<OldDB>().unwrap();
        sqlx::query(
            r#"
SELECT user_id, karma
FROM user_karma
WHERE guild_id = $1
ORDER BY karma DESC
LIMIT $2"#,
        )
        .bind(guild_id.to_string())
        .bind(limit)
        .fetch_all(&*db)
        .await?
    };

    let mut lines = Vec::with_capacity(limit as usize);

    for row in &rows {
        let user_id = row.get::<String, _>(0).parse::<u64>().unwrap();
        let karma: i32 = row.get(1);
        let username = util::get_username(&ctx.http, &usernames, user_id).await;
        lines.push(format!("{} \u{2014} {}\n", username, karma));
    }

    msg.channel_id.say(&ctx.http, lines.concat()).await?;

    Ok(())
}
