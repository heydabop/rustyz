use crate::error::CommandResult;
use serde::Deserialize;
use serenity::all::CommandInteraction;
use serenity::builder::{CreateEmbed, EditInteractionResponse};
use serenity::client::Context;

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
pub async fn affixes(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let client = reqwest::Client::new();
    let affixes = client
        .get("https://raider.io/api/v1/mythic-plus/affixes?region=us&locale=en")
        .send()
        .await?
        .json::<Affixes>()
        .await?;
    if affixes.details.len() < 3 {
        return Err("unexpected response from raider.io".into());
    }

    interaction
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().embed(
                CreateEmbed::new()
                    .title(affixes.title)
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
                    .field(
                        "Xal'atath's Guile (+12)",
                        "Xal'atath betrays players, revoking her bargains and increasing the health and damage of enemies by 20%.",
                        false,
                    ),
            ),
        )
        .await?;

    Ok(())
}
