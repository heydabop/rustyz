use crate::util::record_say;
use chrono::prelude::*;
use chrono_tz::Tz;
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::channel::Message;

#[command]
pub async fn sebbitime(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    record_say(ctx, msg, twelve_hour("Europe/Copenhagen")).await?;
    Ok(())
}

#[command]
pub async fn mirotime(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    record_say(ctx, msg, twelve_hour("Europe/Helsinki")).await?;
    Ok(())
}

#[command]
pub async fn nieltime(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    record_say(ctx, msg, twelve_hour("Europe/Stockholm")).await?;
    Ok(())
}

#[command]
pub async fn birdtime(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    record_say(ctx, msg, twelve_hour("Europe/Oslo")).await?;
    Ok(())
}

#[command]
#[aliases("twintime", "realtime", "natime")]
pub async fn ustime(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    record_say(ctx, msg, twentyfour_hour("America/Chicago")).await?;
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
