use crate::error::CommandResult;
use serenity::all::CommandInteraction;
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;
use std::str;
use tokio::process::Command;

// Replies to msg with a fortune
pub async fn fortune(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let output = Command::new("fortune").arg("-as").output().await?;
    let content = str::from_utf8(&output.stdout)?;
    interaction
        .edit_response(&ctx.http, EditInteractionResponse::new().content(content))
        .await?;

    Ok(())
}
