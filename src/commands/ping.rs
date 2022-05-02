use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::interactions::{
    application_command::ApplicationCommandInteraction, InteractionResponseType,
};

pub async fn ping(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    interaction
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| message.content("pong"))
        })
        .await?;

    Ok(())
}
