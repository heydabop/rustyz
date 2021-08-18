use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::channel::Message;
use std::process::Command;
use std::str;

// Replies to msg with a fortune
#[command]
pub async fn fortune(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    msg.channel_id
        .say(
            &ctx.http,
            str::from_utf8(&Command::new("fortune").arg("-as").output().unwrap().stdout).unwrap(),
        )
        .await?;
    Ok(())
}
