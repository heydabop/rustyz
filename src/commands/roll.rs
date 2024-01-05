use crate::error::CommandResult;
use rand::{thread_rng, Rng};
use serenity::all::{CommandDataOptionValue, CommandInteraction};
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;

pub async fn roll(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let mut sides: u32 = 100;
    if let Some(o) = interaction.data.options.first() {
        if let CommandDataOptionValue::Integer(s) = o.value {
            sides = u32::try_from(s)?;
        }
    }

    let result = {
        let mut rng = thread_rng();
        rng.gen_range(1..=sides)
    };

    interaction
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content(result.to_string()),
        )
        .await?;

    Ok(())
}
