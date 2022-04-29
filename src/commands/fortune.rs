use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::interactions::{
    application_command::ApplicationCommandInteraction, InteractionResponseType,
};
use std::process::Command;
use std::str;

// Replies to msg with a fortune
pub async fn fortune(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    interaction
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message.content(
                        str::from_utf8(
                            &Command::new("fortune").arg("-as").output().unwrap().stdout,
                        )
                        .unwrap(),
                    )
                })
        })
        .await?;

    Ok(())
}
