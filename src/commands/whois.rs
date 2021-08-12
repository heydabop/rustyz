use crate::util;
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandError, CommandResult};
use serenity::model::channel::Message;

// Replies with the username or nickname of the supplied user ID
// Takes a single required argument of a user ID
#[command]
pub async fn whois(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let user_id: u64 = if let Ok(user_id) = args.single() {
        user_id
    } else {
        return Err(CommandError::from("Invalid user ID"));
    };
    let members = util::collect_members(ctx, msg).await;

    let username = util::get_username(&ctx.http, &members, user_id).await;

    msg.channel_id.say(&ctx.http, username).await?;

    Ok(())
}
