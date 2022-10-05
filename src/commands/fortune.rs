use crate::error::CommandResult;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use std::str;
use tokio::process::Command;

// Replies to msg with a fortune
pub async fn fortune(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let output = Command::new("fortune").arg("-as").output().await?;
    let content = str::from_utf8(&output.stdout)?;
    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(content))
        .await?;

    Ok(())
}
