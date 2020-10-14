use crate::DB;
use num_traits::cast::ToPrimitive;
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::channel::Message;
use sqlx::types::Decimal;
use sqlx::Row;
use std::collections::HashMap;

#[command]
#[only_in(guilds)]
pub async fn top(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let channel = match msg.channel(&ctx.cache).await {
        Some(channel) => channel,
        None => ctx.http.get_channel(msg.channel_id.0).await.unwrap(),
    }
    .guild()
    .unwrap();

    // This feels a little clunky (as its also combined with the for loop below)
    // However in testing it seems faster than not mapping and instead hitting guild.member(&ctx) (falling back to http.get_user) for each member
    // Worth making note of tho as it probably doesn't scale well to large guilds with hundreds of members
    let mut usernames: HashMap<u64, String> = HashMap::new();
    for member in channel.members(&ctx.cache).await.unwrap() {
        let username = match member.nick {
            Some(nick) => nick,
            None => member.user.name,
        };
        usernames.insert(member.user.id.0, username);
    }
    let limit: u32 = args.single().unwrap_or(5).min(100);

    let rows = {
        let data = ctx.data.read().await;
        let db = data.get::<DB>().unwrap();
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
        .await
        .unwrap()
    };

    let mut lines = Vec::with_capacity(limit as usize);

    for row in &rows {
        let user_id = row.get::<Decimal, _>(0).to_u64().unwrap();
        let num_messages: i64 = row.get(1);
        let username = match usernames.get(&user_id) {
            Some(username) => username.clone(),
            None => match ctx.http.get_user(user_id).await {
                Ok(user) => user.name,
                Err(_) => String::from("`<UNKNOWN>`"),
            },
        };
        lines.push(format!("{} \u{2014} {}\n", username, num_messages));
    }

    msg.channel_id.say(&ctx.http, lines.concat()).await?;

    Ok(())
}
