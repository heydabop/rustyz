use crate::model::{LastCommandMessages, LastUserPresence};
use regex::Regex;
use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::http::client::Http;
use serenity::model::{
    channel::Message,
    guild::Member,
    id::{GuildId, MessageId, UserId},
    user::OnlineStatus,
};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

pub struct UserIdName {
    pub id: u64,
    pub name: String,
    pub status: Option<OnlineStatus>,
}

// This feels a little clunky (as its also combined with get_username below)
// However in testing it seems faster than not mapping and instead hitting guild.member(&ctx) (falling back to http.get_user) for each member
// Worth making note of tho as it probably doesn't scale well to large guilds with hundreds of members

// Returns a mapping of user IDs to Members
// Panics on an http error or if msg wasn't sent in a GuildChannel
pub async fn collect_members(ctx: &Context, msg: &Message) -> CommandResult<HashMap<u64, Member>> {
    let guild = match msg.channel(&ctx.cache).await {
        Some(channel) => channel,
        None => ctx.http.get_channel(msg.channel_id.0).await?,
    }
    .guild()
    .unwrap();

    let mut members_by_id: HashMap<u64, Member> = HashMap::new();
    let members = match guild.members(&ctx.cache).await {
        Ok(members) => members,
        Err(_) => ctx.http.get_guild_members(guild.id.0, None, None).await?,
    };
    for member in members {
        members_by_id.insert(member.user.id.0, member);
    }

    Ok(members_by_id)
}

// Looks up username/nickname for user_id in usernames, falling back to an http call if the user_id isn't present
pub async fn get_username(
    http: &Arc<Http>,
    members: &HashMap<u64, Member>,
    user_id: u64,
) -> String {
    match members.get(&user_id) {
        Some(member) => match &member.nick {
            Some(nick) => nick.clone(),
            None => member.user.name.clone(),
        },
        None => match http.get_user(user_id).await {
            Ok(user) => user.name,
            Err(_) => String::from("`<UNKNOWN>`"),
        },
    }
}

pub async fn search_user_id_by_name(
    ctx: &Context,
    msg: &Message,
    search: &str,
) -> CommandResult<Option<(u64, String)>> {
    let search = search.to_ascii_lowercase();
    let members = collect_members(ctx, msg).await?;
    let mut similar_usernames: Vec<(u64, String)> = Vec::new();
    for (user_id, member) in members {
        if member.user.name.to_ascii_lowercase().contains(&search) {
            similar_usernames.push((user_id, member.user.name));
        } else if let Some(nick) = member.nick {
            if nick.to_ascii_lowercase().contains(&search) {
                similar_usernames.push((user_id, nick));
            }
        }
    }
    if similar_usernames.is_empty() {
        return Ok(None);
    }
    if similar_usernames.len() == 1 {
        return Ok(Some((
            similar_usernames[0].0,
            similar_usernames[0].1.clone(),
        )));
    }

    let mut best_username = String::new();
    let mut best_user_id = 0;
    let mut best_score = 0.0;
    for (user_id, username) in similar_usernames {
        let lower_username = username.to_ascii_lowercase();
        let score = cosine_similarity(&search, &lower_username);
        if score > best_score {
            best_username = username;
            best_user_id = user_id;
            best_score = score;
        }
    }

    Ok(Some((best_user_id, best_username)))
}

fn cosine_similarity(a_str: &str, b_str: &str) -> f64 {
    let mut a_map: HashMap<u8, u32> = HashMap::new();
    let mut b_map: HashMap<u8, u32> = HashMap::new();
    for byte in a_str.as_bytes() {
        if let Some(count) = a_map.get_mut(byte) {
            *count += 1;
        } else {
            a_map.insert(*byte, 1);
        }
    }
    for byte in b_str.as_bytes() {
        if let Some(count) = b_map.get_mut(byte) {
            *count += 1;
        } else {
            b_map.insert(*byte, 1);
        }
    }

    let dot_product = a_map.iter().fold(0.0, |dot, (key, val)| {
        dot + f64::from(*val) * b_map.get(key).map_or(0.0, |b_val| f64::from(*b_val))
    });

    let a_magnitude = a_map
        .values()
        .fold(0.0, |mag, val| mag + f64::from(*val).powi(2));
    let b_magnitude = b_map
        .values()
        .fold(0.0, |mag, val| mag + f64::from(*val).powi(2));
    let magnitude = a_magnitude.sqrt() * b_magnitude.sqrt();

    if magnitude < f64::EPSILON {
        0.0
    } else {
        dot_product / magnitude
    }
}

pub async fn record_say(
    ctx: &Context,
    msg: &Message,
    content: impl std::fmt::Display,
) -> serenity::Result<Message> {
    let reply = msg.channel_id.say(&ctx.http, content).await?;

    let last_messages = {
        ctx.data
            .read()
            .await
            .get::<LastCommandMessages>()
            .unwrap()
            .clone()
    };

    {
        let mut last_messages = last_messages.write().await;
        last_messages.insert((msg.channel_id, msg.author.id), [msg.id, reply.id]);
    }

    Ok(reply)
}

pub async fn record_sent_message(ctx: &Context, source_msg: &Message, reply_id: MessageId) {
    let last_messages = {
        ctx.data
            .read()
            .await
            .get::<LastCommandMessages>()
            .unwrap()
            .clone()
    };

    {
        let mut last_messages = last_messages.write().await;
        last_messages.insert(
            (source_msg.channel_id, source_msg.author.id),
            [source_msg.id, reply_id],
        );
    }
}

pub async fn user_from_mention(
    ctx: &Context,
    msg: &Message,
    args: &str,
) -> CommandResult<Option<UserIdName>> {
    if args.is_empty() {
        return Ok(None);
    }
    let mut username: Option<String> = None;
    let mut status: Option<OnlineStatus> = None;
    let mention_regex = Regex::new(r#"^\s*<@!?(\d+?)>\s*$"#).unwrap();
    return if let Some(captures) = mention_regex.captures(args) {
        let user_id = if let Ok(user_id) = u64::from_str(captures.get(1).unwrap().as_str()) {
            user_id
        } else {
            record_say(ctx, msg, "```Invalid mention```").await?;
            return Ok(None);
        };
        if let Some(guild) = ctx.cache.guild(msg.guild_id.unwrap()).await {
            if let Ok(member) = guild.member(ctx, user_id).await {
                username = member.nick;
            }
            if let Some(presence) = guild.presences.get(&UserId(user_id)) {
                status = Some(presence.status);
            }
        }
        if username.is_none() {
            let members = collect_members(ctx, msg).await?;
            username = if let Some(member) = members.get(&user_id) {
                match &member.nick {
                    Some(nick) => Some(nick.clone()),
                    None => Some(member.user.name.clone()),
                }
            } else {
                record_say(ctx, msg, "```Unable to find user```").await?;
                return Ok(None);
            };
        }
        if status.is_none() {
            status = get_user_status(ctx, msg.guild_id.unwrap(), UserId(user_id)).await;
        }
        Ok(Some(UserIdName {
            id: user_id,
            name: username.unwrap(),
            status,
        }))
    } else if let Some(user) = search_user_id_by_name(ctx, msg, args).await? {
        Ok(Some(UserIdName {
            id: user.0,
            name: user.1,
            status: get_user_status(ctx, msg.guild_id.unwrap(), UserId(user.0)).await,
        }))
    } else {
        record_say(ctx, msg, "```Unable to find user```").await?;
        Ok(None)
    };
}

async fn get_user_status(
    ctx: &Context,
    guild_id: GuildId,
    user_id: UserId,
) -> Option<OnlineStatus> {
    let last_presence = {
        let data = ctx.data.read().await;
        data.get::<LastUserPresence>().unwrap().clone()
    };
    if let Some(last_presence) = last_presence.read().await.get(&user_id) {
        return Some(last_presence.status);
    }
    if let Some(presences) = ctx
        .cache
        .guild_field(guild_id, |g| g.presences.clone())
        .await
    {
        if let Some(presence) = presences.get(&user_id) {
            return Some(presence.status);
        }
    }
    None
}
