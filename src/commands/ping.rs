use crate::error::CommandResult;
use serenity::all::CommandInteraction;
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;

pub async fn ping(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    interaction
        .edit_response(&ctx.http, EditInteractionResponse::new().content("pong"))
        .await?;

    Ok(())
}
