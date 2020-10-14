use serenity::client::Context;
use serenity::http::client::Http;
use serenity::model::channel::Message;
use std::collections::HashMap;
use std::sync::Arc;

// This feels a little clunky (as its also combined with get_username below)
// However in testing it seems faster than not mapping and instead hitting guild.member(&ctx) (falling back to http.get_user) for each member
// Worth making note of tho as it probably doesn't scale well to large guilds with hundreds of members

// Returns a mapping of user IDs to nicknames or usernames (if no nick in guild)
// Panics on an http error or if msg wasn't sent in a GuildChannel
pub async fn collect_usernames(ctx: &Context, msg: &Message) -> HashMap<u64, String> {
    let channel = match msg.channel(&ctx.cache).await {
        Some(channel) => channel,
        None => ctx.http.get_channel(msg.channel_id.0).await.unwrap(),
    }
    .guild()
    .unwrap();

    let mut usernames: HashMap<u64, String> = HashMap::new();
    for member in channel.members(&ctx.cache).await.unwrap() {
        let username = match member.nick {
            Some(nick) => nick,
            None => member.user.name,
        };
        usernames.insert(member.user.id.0, username);
    }

    usernames
}

// Looks up username/nickname for user_id in usernames, falling back to an http call if the user_id isn't present
pub async fn get_username(
    http: &Arc<Http>,
    usernames: &HashMap<u64, String>,
    user_id: u64,
) -> String {
    match usernames.get(&user_id) {
        Some(username) => username.clone(),
        None => match http.get_user(user_id).await {
            Ok(user) => user.name,
            Err(_) => String::from("`<UNKNOWN>`"),
        },
    }
}
