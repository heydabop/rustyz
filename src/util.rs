use crate::error::CommandError;
use crate::model::LastUserPresence;
use serenity::client::Context;
use serenity::http::Http;
use serenity::model::{
    guild::Member,
    id::{GuildId, UserId},
    user::OnlineStatus,
};
use std::collections::HashMap;
use std::sync::Arc;

// This feels a little clunky (as its also combined with get_username below)
// However in testing it seems faster than not mapping and instead hitting guild.member(&ctx) (falling back to http.get_user) for each member
// Worth making note of tho as it probably doesn't scale well to large guilds with hundreds of members

// Returns a mapping of user IDs to Members
pub async fn collect_members_guild_id(
    ctx: &Context,
    guild_id: GuildId,
) -> Result<HashMap<UserId, Member>, CommandError> {
    let guild_members = {
        if let Some(g) = ctx.cache.guild(guild_id) {
            Some(g.members.clone())
        } else {
            None
        }
    };
    let members_by_id: HashMap<UserId, Member> = match guild_members {
        Some(gm) => gm,
        None => {
            let guild = ctx.http.get_guild(guild_id).await?;
            let members: Vec<Member> = guild.members(&ctx.http, None, None).await?;
            members.into_iter().map(|m| (m.user.id, m)).collect()
        }
    };

    Ok(members_by_id)
}

pub async fn get_username_userid(
    http: &Arc<Http>,
    members: &HashMap<UserId, Member>,
    user_id: UserId,
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

pub async fn get_user_status(
    ctx: &Context,
    guild_id: GuildId,
    user_id: UserId,
) -> Option<OnlineStatus> {
    let last_presence = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<LastUserPresence>().unwrap().clone()
    };
    if let Some(last_presence) = last_presence.read().await.get(&user_id) {
        return Some(last_presence.status);
    }
    if let Some(guild) = ctx.cache.guild(guild_id) {
        if let Some(presence) = guild.presences.get(&user_id) {
            return Some(presence.status);
        }
    }
    None
}
