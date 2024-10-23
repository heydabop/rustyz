use crate::error::CommandResult;
use serenity::all::CommandInteraction;
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;
use serenity::model::Permissions;

pub async fn invite(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let permissions: u64 = (Permissions::VIEW_CHANNEL
        | Permissions::USE_APPLICATION_COMMANDS
        | Permissions::SEND_MESSAGES
        | Permissions::READ_MESSAGE_HISTORY)
        .bits();
    let url = format!(
        "https://discord.com/api/oauth2/authorize?client_id={}&permissions={permissions}&scope=bot%20applications.commands",
        interaction.application_id
    );
    interaction
        .edit_response(&ctx.http, EditInteractionResponse::new().content(url))
        .await?;
    Ok(())
}
