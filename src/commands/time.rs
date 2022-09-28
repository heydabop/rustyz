use crate::error::CommandResult;
use chrono::prelude::*;
use chrono_tz::{ParseError, Tz};
use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;

pub async fn time(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    tz: &str,
) -> CommandResult {
    let content = if tz == "America/Chicago" {
        twentyfour_hour(tz)
    } else {
        twelve_hour(tz)
    }?;

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(content))
        .await?;

    Ok(())
}

fn twelve_hour(iana: &str) -> Result<String, ParseError> {
    let tz: Tz = iana.parse()?;
    let now = Local::now().with_timezone(&tz);
    Ok(now.format("%I:%M %p - %a, %b %d").to_string())
}

fn twentyfour_hour(iana: &str) -> Result<String, ParseError> {
    let tz: Tz = iana.parse()?;
    let now = Local::now().with_timezone(&tz);
    Ok(now.format("%H:%M - %a, %b %d").to_string())
}
