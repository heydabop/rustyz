use crate::error::CommandResult;
use rand::{thread_rng, Rng};
use serenity::{
    all::{CommandDataOptionValue, CommandInteraction},
    builder::EditInteractionResponse,
    client::Context,
};

pub async fn zalgo(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let input = if let CommandDataOptionValue::String(i) = &interaction.data.options[0].value {
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
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content(message.iter().collect::<String>()),
        )
        .await?;

    Ok(())
}
