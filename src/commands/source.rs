use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;

pub async fn source(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    interaction
        .edit_original_interaction_response(&ctx.http, |response| {
            response.content("https://github.com/heydabop/rustyz")
        })
        .await?;

    Ok(())
}
