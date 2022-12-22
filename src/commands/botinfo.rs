use crate::error::CommandResult;
use crate::model::StartInstant;
use chrono::naive::NaiveDateTime;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use std::str;
use std::time::Instant;
use tokio::process::Command;

pub async fn botinfo(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let guild_id = match interaction.guild_id {
        Some(g) => g,
        None => {
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content("Command can only be used in a server")
                })
                .await?;
            return Ok(());
        }
    };

    let bot = ctx.cache.current_user();
    let member = ctx.http.get_member(guild_id.0, bot.id.0).await?;
    let user = ctx.http.get_user(bot.id.0).await?;
    let num_guilds = bot.guilds(&ctx.http).await?.len();
    let uptime_output = Command::new("uptime").arg("-p").output().await?;
    let server_uptime = str::from_utf8(&uptime_output.stdout)?[3..].replace(", ", "\n");
    let since_start = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        let start = data.get::<StartInstant>().unwrap();
        Instant::now().duration_since(*start).as_secs()
    };
    let mut bot_uptime = vec![];
    let weeks = since_start / 604_800;
    let days = (since_start % 604_800) / 86_400;
    let hours = (since_start % 86_400) / 3600;
    let minutes = (since_start % 3600) / 60;
    let seconds = since_start % 60;
    if weeks > 0 {
        bot_uptime.push(format!(
            "{} week{}",
            weeks,
            if weeks == 1 { "" } else { "s" }
        ));
    }
    if days > 0 {
        bot_uptime.push(format!("{days} day{}", if days == 1 { "" } else { "s" }));
    }
    if hours > 0 {
        bot_uptime.push(format!(
            "{} hour{}",
            hours,
            if hours == 1 { "" } else { "s" }
        ));
    }
    if minutes > 0 {
        bot_uptime.push(format!(
            "{} minute{}",
            minutes,
            if minutes == 1 { "" } else { "s" }
        ));
    }
    if seconds > 0 {
        bot_uptime.push(format!(
            "{} second{}",
            seconds,
            if seconds == 1 { "" } else { "s" }
        ));
    }

    interaction
        .edit_original_interaction_response(&ctx.http, |r| {
            r.embed(|e| {
                e.title(member.nick.as_ref().unwrap_or(&bot.tag()))
                    .thumbnail(member.face())
                    .timestamp(serenity::model::timestamp::Timestamp::now())
                    .field(
                        "Joined Discord",
                        NaiveDateTime::from_timestamp(user.created_at().unix_timestamp(), 0)
                            .format("%b %e, %Y")
                            .to_string(),
                        true,
                    )
                    .field(
                        "Joined Server",
                        if let Some(joined_at) = member.joined_at {
                            NaiveDateTime::from_timestamp(joined_at.unix_timestamp(), 0)
                                .format("%b %e, %Y")
                                .to_string()
                        } else {
                            String::from("`Unknown`")
                        },
                        true,
                    )
                    .field("Member of", format!("{num_guilds} servers"), true)
                    .field("Host Uptime", server_uptime, true)
                    .field("Bot Uptime", bot_uptime.join("\n"), true);
                if member.nick.is_some() {
                    e.description(bot.tag());
                }
                if let Some(banner) = user.banner_url() {
                    e.image(banner);
                }
                if let Some(color) = user.accent_colour {
                    e.color(color);
                } else if let Some(member_color) = member.colour(&ctx.cache) {
                    e.color(member_color);
                }
                e
            })
        })
        .await?;

    Ok(())
}
