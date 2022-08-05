use rand::{thread_rng, Rng};
use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::application::interaction::{
    application_command::{ApplicationCommandInteraction, CommandDataOptionValue},
    InteractionResponseType,
};

pub async fn roll(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    interaction
        .create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
        })
        .await?;

    let sides: u32 = interaction
        .data
        .options
        .get(0)
        .and_then(|o| {
            o.resolved.as_ref().map(|r| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                if let CommandDataOptionValue::Integer(s) = r {
                    *s as u32
                } else {
                    100
                }
            })
        })
        .unwrap_or(100);

    let result = {
        let mut rng = thread_rng();
        rng.gen_range(1..=sides)
    };

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(result))
        .await?;

    Ok(())
}
