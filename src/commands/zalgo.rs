use rand::{thread_rng, Rng};
use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};

pub async fn zalgo(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let input = if let CommandDataOptionValue::String(i) =
        match interaction.data.options[0].resolved.as_ref() {
            Some(o) => o,
            None => return Err("Missing input".into()),
        } {
        i.chars()
    } else {
        return Err("Missing input".into());
    };

    let mut message: Vec<char> = vec![];

    {
        let mut rng = thread_rng();
        #[allow(clippy::unwrap_used)]
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
