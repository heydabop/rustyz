use rand::{thread_rng, Rng};
use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::interactions::{
    application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue,
    },
    InteractionResponseType,
};

pub async fn roll(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let sides: u32 = interaction
        .data
        .options
        .get(0)
        .and_then(|o| {
            o.resolved.as_ref().map(|r| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                if let ApplicationCommandInteractionDataOptionValue::Integer(s) = r {
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
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| message.content(result))
        })
        .await?;

    Ok(())
}
