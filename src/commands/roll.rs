use crate::error::CommandResult;
use rand::{thread_rng, Rng};
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};

pub async fn roll(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let mut sides: u32 = 100;
    if let Some(o) = interaction.data.options.first() {
        if let Some(CommandDataOptionValue::Integer(s)) = o.resolved {
            sides = u32::try_from(s)?;
        }
    }

    let result = {
        let mut rng = thread_rng();
        rng.gen_range(1..=sides)
    };

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(result))
        .await?;

    Ok(())
}
