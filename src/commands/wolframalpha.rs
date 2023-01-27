use crate::config;
use crate::error::CommandResult;
use reqwest::Url;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::channel::AttachmentType;
use std::borrow::Cow;

// Replies with image from Wolfram Alpha Simple API
// Takes a single required argument: input query
pub async fn simple(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let CommandDataOptionValue::String(input) = (match interaction.data.options[0].resolved.as_ref() {
        Some(i) => i,
        None => return Err("Missing required input query".into()),
    }) else {
        return Err("Non-string input query".into());
    };

    let wolfram_alpha_app_id = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<config::WolframAlpha>().unwrap().app_id.clone()
    };

    let url = Url::parse_with_params(
        "https://api.wolframalpha.com/v1/simple",
        &[("appid", &wolfram_alpha_app_id), ("i", input)],
    )?;
    let image_bytes = reqwest::get(url).await?.bytes().await?;

    interaction
        .create_followup_message(&ctx.http, |m| {
            m.add_file(AttachmentType::Bytes {
                data: Cow::from(image_bytes.to_vec()),
                filename: "wa.gif".to_string(),
            })
        })
        .await?;

    Ok(())
}
