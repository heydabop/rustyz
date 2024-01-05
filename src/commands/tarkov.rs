use crate::config::TarkovMarket;
use crate::error::CommandResult;
use num_format::{Locale, ToFormattedString};
use reqwest::Url;
use serde::Deserialize;
use serenity::all::{CommandDataOptionValue, CommandInteraction};
use serenity::builder::{CreateEmbed, EditInteractionResponse};
use serenity::client::Context;
use serenity::model::Timestamp;

#[derive(Deserialize)]
struct Item {
    name: String,
    price: u32,
    #[serde(rename = "avg24hPrice")]
    avg_24_hour_price: u32,
    #[serde(rename = "avg7daysPrice")]
    avg_7_day_price: u32,
    #[serde(rename = "traderName")]
    trader_name: String,
    #[serde(rename = "traderPrice")]
    trader_price: u32,
    #[serde(rename = "traderPriceCur")]
    trader_price_currency: String,
    updated: String,
    #[serde(rename = "diff24h")]
    diff_24_hour: f32,
    #[serde(rename = "diff7days")]
    diff_7_day: f32,
    icon: String,
    link: String,
}

// Searches the Tarkov Market site for an item with the provided name, returning flea market and vendor info
pub async fn tarkov(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let search = interaction.data.options.first().map_or("", |o| {
        if let CommandDataOptionValue::String(s) = &o.value {
            s.as_str()
        } else {
            ""
        }
    });

    let api_key = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<TarkovMarket>().unwrap().api_key.clone()
    };

    let client = reqwest::Client::new();
    let items: Vec<Item> = client
        .get(Url::parse_with_params(
            "https://tarkov-market.com/api/v1/item",
            &[("q", search)],
        )?)
        .header("x-api-key", api_key)
        .send()
        .await?
        .json()
        .await?;

    if items.is_empty() {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("No items found"),
            )
            .await?;
        return Ok(());
    }
    let item = &items[0];

    let trader_price = if item.trader_price_currency == "$" {
        format!("${}", item.trader_price.to_formatted_string(&Locale::en))
    } else {
        format!(
            "{} {}",
            item.trader_price.to_formatted_string(&Locale::en),
            item.trader_price_currency
        )
    };

    let mut embed = CreateEmbed::new()
        .title(&item.name)
        .url(&item.link)
        .field(
            "Last Lowest",
            format!("{} \u{20bd}", item.price.to_formatted_string(&Locale::en)),
            true,
        )
        .field(
            "24h Avg",
            format!(
                "{} \u{20bd}",
                item.avg_24_hour_price.to_formatted_string(&Locale::en)
            ),
            true,
        )
        .field(
            "7d Avg",
            format!(
                "{} \u{20bd}",
                item.avg_7_day_price.to_formatted_string(&Locale::en)
            ),
            true,
        )
        .field("\u{200B}", "\u{200B}", false)
        .field(
            "24h Diff",
            format!(
                "{}{}%",
                if item.diff_24_hour > 0.0 { "+" } else { "" },
                item.diff_24_hour
            ),
            true,
        )
        .field(
            "7d Diff",
            format!(
                "{}{}%",
                if item.diff_7_day > 0.0 { "+" } else { "" },
                item.diff_7_day
            ),
            true,
        )
        .field("\u{200B}", "\u{200B}", false)
        .field(&item.trader_name, trader_price, false)
        .thumbnail(&item.icon);

    if let Ok(updated) = Timestamp::parse(&item.updated) {
        embed = embed.timestamp(updated);
    }

    interaction
        .edit_response(&ctx.http, EditInteractionResponse::new().embed(embed))
        .await?;

    Ok(())
}
