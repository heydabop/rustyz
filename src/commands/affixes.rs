use reqwest::Url;
use serde::Deserialize;
use serenity::client::Context;
use serenity::framework::standard::{macros::command, CommandResult};
use serenity::model::channel::Message;

#[derive(Debug, Deserialize)]
struct Affixes {
    title: String,
    #[serde(rename = "affix_details")]
    details: Vec<Affix>,
}

#[derive(Debug, Deserialize)]
struct Affix {
    name: String,
    description: String,
}

// Returns this week's M+ affixes for US
#[command]
#[aliases("affix")]
pub async fn affixes(ctx: &Context, msg: &Message) -> CommandResult {
    let client = reqwest::Client::new();
    let affixes = client
        .get(
            Url::parse("https://raider.io/api/v1/mythic-plus/affixes?region=us&locale=en").unwrap(),
        )
        .send()
        .await?
        .json::<Affixes>()
        .await?;

    msg.channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title(affixes.title)
                    .url("https://mythicpl.us/")
                    .field(
                        &affixes.details[0].name,
                        &affixes.details[0].description,
                        false,
                    )
                    .field(
                        format!("{} (+4)", affixes.details[1].name),
                        &affixes.details[1].description,
                        false,
                    )
                    .field(
                        format!("{} (+7)", affixes.details[2].name),
                        &affixes.details[2].description,
                        false,
                    )
                    .field(
                        format!("{} (+10)", affixes.details[3].name),
                        &affixes.details[3].description,
                        false,
                    )
            })
        })
        .await?;

    Ok(())
}
