use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use std::process::Command;
use std::str;

// Replies to msg with a fortune
pub async fn fortune(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    interaction
        .edit_original_interaction_response(&ctx.http, |response| {
            response.content(
                str::from_utf8(&Command::new("fortune").arg("-as").output().unwrap().stdout)
                    .unwrap(),
            )
        })
        .await?;

    Ok(())
}
