use crate::config;
use crate::error::CommandResult;
use reqwest::Url;
use serenity::all::{CommandDataOptionValue, CommandInteraction};
use serenity::builder::{
    CreateAttachment, CreateInteractionResponseFollowup, EditInteractionResponse,
};
use serenity::client::Context;
use std::borrow::Cow;

// Replies with image from Wolfram Alpha Simple API
// Takes a single required argument: input query
pub async fn simple(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let CommandDataOptionValue::String(input) = &interaction.data.options[0].value else {
        return Err("Non-string input query".into());
    };

    let wolfram_alpha_app_id = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<config::WolframAlpha>().unwrap().app_id.clone()
    };

    let url = Url::parse_with_params("https://api.wolframalpha.com/v1/simple", &[
        ("appid", &wolfram_alpha_app_id[..]),
        ("units", "imperial"),
        ("i", input),
    ])?;
    let response = reqwest::get(url).await?;
    if let Err(e) = response.error_for_status_ref() {
        if response.status() == 501 {
            interaction
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content(format!(
                        "\u{26A0} `No suitable answer found for \"{input}\"`"
                    )),
                )
                .await?;
            return Ok(());
        }
        return Err(e.into());
    }
    let image_bytes = response.bytes().await?;

    interaction
        .create_followup(
            &ctx.http,
            CreateInteractionResponseFollowup::new().add_file(CreateAttachment::bytes(
                Cow::from(image_bytes.to_vec()),
                "wa.gif".to_string(),
            )),
        )
        .await?;

    Ok(())
}

// Replies with single line of text from Wolfram Alpha Short API
// Takes a single required argument: input query
pub async fn short(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let CommandDataOptionValue::String(input) = &interaction.data.options[0].value else {
        return Err("Non-string input query".into());
    };

    let wolfram_alpha_app_id = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<config::WolframAlpha>().unwrap().app_id.clone()
    };

    let url = Url::parse_with_params("https://api.wolframalpha.com/v1/result", &[
        ("appid", &wolfram_alpha_app_id[..]),
        ("units", "imperial"),
        ("i", input),
    ])?;
    let response = reqwest::get(url).await?;
    if let Err(e) = response.error_for_status_ref() {
        if response.status() == 501 {
            interaction
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content(format!(
                        "\u{26A0} `No suitable answer found for \"{input}\"`"
                    )),
                )
                .await?;
            return Ok(());
        }
        return Err(e.into());
    }
    let answer = response.text().await?;

    interaction
        .create_followup(
            &ctx.http,
            CreateInteractionResponseFollowup::new().content(answer),
        )
        .await?;

    Ok(())
}
