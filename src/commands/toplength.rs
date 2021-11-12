use crate::model::OldDB;
use crate::util;
use num_traits::cast::ToPrimitive;
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::channel::Message;
use sqlx::types::Decimal;
use sqlx::Row;
use std::collections::HashMap;

// Replies to msg with users in channel sorted by average length of sent messages
// Allows a single optional arg of how many users to list, defaults to 5
#[command]
#[only_in(guilds)]
pub async fn toplength(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let members = util::collect_members(ctx, msg).await?;
    let limit: usize = args.single().unwrap_or(5).min(100);

    let rows = {
        let data = ctx.data.read().await;
        let db = data.get::<OldDB>().unwrap();
        sqlx::query(
            r#"
SELECT author_id, content
FROM message
WHERE chan_id = $1
AND content NOT LIKE '/%'"#,
        )
        .bind(Decimal::from(msg.channel_id.0))
        .fetch_all(&*db)
        .await?
    };

    let mut messages_per_user: HashMap<u64, u64> = HashMap::new();
    let mut words_per_user: HashMap<u64, usize> = HashMap::new();

    for row in &rows {
        let user_id = row.get::<Decimal, _>(0).to_u64().unwrap();
        let message = row.get::<String, _>(1);
        let num_words = message.split(' ').count();
        if let Some(messages) = messages_per_user.get_mut(&user_id) {
            *messages += 1;
        } else {
            messages_per_user.insert(user_id, 1);
        }
        if let Some(words) = words_per_user.get_mut(&user_id) {
            *words += num_words;
        } else {
            words_per_user.insert(user_id, num_words);
        }
    }

    let mut avg_per_user: Vec<(String, f64)> = vec![];
    for (user_id, messages) in &messages_per_user {
        let words = words_per_user.get(user_id).unwrap();
        let username = util::get_username(&ctx.http, &members, *user_id).await;
        #[allow(clippy::cast_precision_loss)]
        avg_per_user.push((username, *words as f64 / *messages as f64));
    }
    avg_per_user.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    avg_per_user.truncate(limit);

    let lines: Vec<String> = avg_per_user
        .iter()
        .map(|u| format!("{} \u{2014} {:.2}\n", u.0, u.1))
        .collect();

    util::record_say(ctx, msg, lines.concat()).await?;

    Ok(())
}
