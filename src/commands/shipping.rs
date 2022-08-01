use crate::{config, shippo};
use serenity::client::Context;
use serenity::framework::standard::{CommandError, CommandResult};
use serenity::model::application::interaction::{
    application_command::{ApplicationCommandInteraction, CommandDataOptionValue},
    InteractionResponseType,
};

pub async fn track(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let tracking_number = if let CommandDataOptionValue::String(n) =
        interaction.data.options[1].resolved.as_ref().unwrap()
    {
        if let CommandDataOptionValue::String(company) =
            interaction.data.options[0].resolved.as_ref().unwrap()
        {
            #[allow(clippy::enum_glob_use)]
            use shippo::TrackingNumber::*;
            let number = String::from(n);
            match company.as_str() {
                "fedex" => FedEx(number),
                "ups" => Ups(number),
                "usps" => Usps(number),
                &_ => {
                    return Err(CommandError::from(format!(
                        "Unrecognized company: {}",
                        company
                    )))
                }
            }
        } else {
            return Err(CommandError::from("Missing company arg"));
        }
    } else {
        return Err(CommandError::from("Missing tracking number arg"));
    };

    interaction
        .create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
        })
        .await?;

    let shippo_api_key = {
        let data = ctx.data.read().await;
        data.get::<config::Shippo>().unwrap().api_key.clone()
    };

    let shipment = shippo::get_tracking_status(&tracking_number, &shippo_api_key).await?;

    let eta_string = if let Some(eta) = shipment.eta {
        format!("\nETA: {}", eta.format("%A, %b %d"))
    } else {
        String::new()
    };

    interaction
        .edit_original_interaction_response(&ctx.http, |response| {
            response.content(format!(
                "{}{}",
                shipment.tracking_status.status_details, eta_string
            ))
        })
        .await?;

    Ok(())
}
