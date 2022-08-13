use rand::{thread_rng, Rng};
use serenity::client::Context;
use serenity::framework::standard::{CommandError, CommandResult};
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};

pub async fn zalgo(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let input = if let CommandDataOptionValue::String(i) =
        interaction.data.options[0].resolved.as_ref().unwrap()
    {
        i.chars()
    } else {
        return Err(CommandError::from("Missing input"));
    };

    let mut message: Vec<char> = vec![];

    {
        let mut rng = thread_rng();
        for c in input {
            message.push(c);
            message.push(char::from_u32(rng.gen_range(0x300..=0x369)).unwrap());
            message.push(char::from_u32(rng.gen_range(0x300..=0x369)).unwrap());
        }
    }

    interaction
        .edit_original_interaction_response(&ctx.http, |response| {
            response.content(message.iter().collect::<String>())
        })
        .await?;

    Ok(())
}
