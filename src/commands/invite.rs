use crate::error::CommandResult;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::Permissions;

pub async fn invite(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let permissions: u64 = (Permissions::VIEW_CHANNEL
        | Permissions::USE_SLASH_COMMANDS
        | Permissions::SEND_MESSAGES
        | Permissions::READ_MESSAGE_HISTORY)
        .bits();
    let url = format!("https://discord.com/api/oauth2/authorize?client_id={}&permissions={}&scope=bot%20applications.commands", interaction.application_id.0, permissions);
    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(url))
        .await?;
    Ok(())
}
