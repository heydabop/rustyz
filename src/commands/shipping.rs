use crate::error::CommandResult;
use crate::model::DB;
use crate::shippo::{Status, TrackingNumber::*};
use crate::{config, shippo};
use serenity::all::{CommandDataOptionValue, CommandInteraction};
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;

pub async fn track(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let mut number = "";
    let mut carrier = "";
    let mut comment = None;
    for o in &interaction.data.options {
        match &o.name[..] {
            "carrier" => {
                if let CommandDataOptionValue::String(c) = &o.value {
                    carrier = c;
                }
            }
            "number" => {
                if let CommandDataOptionValue::String(n) = &o.value {
                    number = n;
                }
            }
            "comment" => {
                if let CommandDataOptionValue::String(c) = &o.value {
                    comment = Some(c);
                }
            }
            _ => {}
        }
    }
    let tracking_number = match carrier {
        "fedex" => FedEx(number.to_string()),
        "ups" => Ups(number.to_string()),
        "usps" => Usps(number.to_string()),
        &_ => return Err(format!("Unrecognized carrier: {carrier}").into()),
    };

    let shippo_api_key = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<config::Shippo>().unwrap().api_key.clone()
    };
    let shipment = shippo::get_tracking_status(&tracking_number, &shippo_api_key).await?;

    let eta_string = if let Some(eta) = shipment.eta {
        format!("\nETA: {}", eta.format("%A, %b %d"))
    } else {
        String::new()
    };

    let status_string = if let Some(status) = shipment.tracking_status {
        if status.status != Status::Delivered {
            let data = ctx.data.read().await;
            #[allow(clippy::unwrap_used)]
            let db = data.get::<DB>().unwrap();
            #[allow(clippy::panic)]
            sqlx::query!("INSERT INTO shipment(carrier, tracking_number, author_id, channel_id, status, comment) VALUES ($1::shipment_carrier, $2, $3, $4, $5::shipment_tracking_status, $6) ON CONFLICT ON CONSTRAINT shipment_uk_carrier_number DO NOTHING", tracking_number.carrier() as _, tracking_number.number(), i64::try_from(interaction.user.id)?, i64::try_from(interaction.channel_id)?, format!("{}", status.status) as _, comment).execute(db).await?;
        }
        status.status_details
    } else {
        String::from("Status Unknown")
    };

    interaction
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content(format!("{status_string}{eta_string}")),
        )
        .await?;

    Ok(())
}
