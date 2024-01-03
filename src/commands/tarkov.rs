use crate::config::TarkovMarket;
use crate::error::CommandResult;
use num_format::{Locale, ToFormattedString};
use reqwest::Url;
use serde::Deserialize;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};

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
pub async fn tarkov(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let search = interaction
        .data
        .options
        .first()
        .and_then(|o| {
            o.resolved.as_ref().map(|r| {
                if let CommandDataOptionValue::String(s) = r {
                    s
                } else {
                    ""
                }
            })
        })
        .unwrap_or("");

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
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content("No items found")
            })
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

    interaction
        .edit_original_interaction_response(&ctx.http, |response| {
            response.embed(|e| {
                e.title(&item.name)
                    .url(&item.link)
                    .timestamp(item.updated.clone())
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
                    .thumbnail(&item.icon)
            })
        })
        .await?;

    Ok(())
}
