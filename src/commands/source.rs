use crate::error::CommandResult;
use serenity::all::CommandInteraction;
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;

pub async fn source(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    interaction
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content("https://github.com/heydabop/rustyz"),
        )
        .await?;

    Ok(())
}
