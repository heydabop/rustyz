use serenity::client::Context;
use serenity::framework::standard::{macros::command, CommandResult};
use serenity::model::channel::Message;

#[command]
pub async fn source(ctx: &Context, msg: &Message) -> CommandResult {
    crate::util::record_say(ctx, msg, "https://github.com/heydabop/rustyz").await?;
    Ok(())
}
