use crate::model::DB;
use crate::shippo::Status;
use crate::{config, shippo};
use serenity::client::Context;
use serenity::framework::standard::{CommandError, CommandResult};
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
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

    let status_string = if let Some(status) = shipment.tracking_status {
        if status.status != Status::Delivered {
            let data = ctx.data.read().await;
            let db = data.get::<DB>().unwrap();
            #[allow(clippy::cast_possible_wrap)]
            sqlx::query!("INSERT INTO shipment(carrier, tracking_number, author_id, channel_id, status) VALUES ($1::shipment_carrier, $2, $3, $4, $5::shipment_tracking_status) ON CONFLICT ON CONSTRAINT shipment_uk_carrier_number DO NOTHING", tracking_number.carrier() as _, tracking_number.number(), interaction.user.id.0 as i64, interaction.channel_id.0 as i64, format!("{}", status.status) as _).execute(db).await?;
        }
        status.status_details
    } else {
        String::from("Status Unknown")
    };

    interaction
        .edit_original_interaction_response(&ctx.http, |response| {
            response.content(format!("{}{}", status_string, eta_string))
        })
        .await?;

    Ok(())
}
