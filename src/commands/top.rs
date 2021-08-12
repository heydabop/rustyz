use crate::util;
use crate::OldDB;
use num_traits::cast::ToPrimitive;
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::channel::Message;
use sqlx::types::Decimal;
use sqlx::Row;

// Replies to msg with the top users in channel sorted by most messages sent
// Allows a single optional arg of how many users to list, defaults to 5
#[command]
#[only_in(guilds)]
pub async fn top(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let members = util::collect_members(ctx, msg).await;
    let limit: u32 = args.single().unwrap_or(5).min(100);

    let rows = {
        let data = ctx.data.read().await;
        let db = data.get::<OldDB>().unwrap();
        sqlx::query(
            r#"
SELECT author_id, count(author_id) AS num_messages
FROM message
WHERE chan_id = $1
AND content NOT LIKE '/%'
GROUP BY author_id
ORDER BY count(author_id) DESC
LIMIT $2"#,
        )
        .bind(Decimal::from(msg.channel_id.0))
        .bind(limit)
        .fetch_all(&*db)
        .await?
    };

    let mut lines = Vec::with_capacity(limit as usize);

    for row in &rows {
        let user_id = row.get::<Decimal, _>(0).to_u64().unwrap();
        let num_messages: i64 = row.get(1);
        let username = util::get_username(&ctx.http, &members, user_id).await;
        lines.push(format!("{} \u{2014} {}\n", username, num_messages));
    }

    msg.channel_id.say(&ctx.http, lines.concat()).await?;

    Ok(())
}
