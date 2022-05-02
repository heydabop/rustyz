use chrono::prelude::*;
use chrono_tz::Tz;
use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::interactions::{
    application_command::ApplicationCommandInteraction, InteractionResponseType,
};

pub async fn time(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    tz: &str,
) -> CommandResult {
    let content = if tz == "America/Chicago" {
        twentyfour_hour(tz)
    } else {
        twelve_hour(tz)
    };

    interaction
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| message.content(content))
        })
        .await?;

    Ok(())
}

fn twelve_hour(iana: &str) -> String {
    let tz: Tz = iana.parse().unwrap();
    let now = Local::now().with_timezone(&tz);
    now.format("%I:%M %p - %a, %b %d").to_string()
}

fn twentyfour_hour(iana: &str) -> String {
    let tz: Tz = iana.parse().unwrap();
    let now = Local::now().with_timezone(&tz);
    now.format("%H:%M - %a, %b %d").to_string()
}
