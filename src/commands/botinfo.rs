use crate::error::{CommandError, CommandResult};
use crate::model::StartInstant;
use chrono::naive::NaiveDateTime;
use serenity::all::CommandInteraction;
use serenity::builder::{CreateEmbed, EditInteractionResponse};
use serenity::client::Context;
use std::str;
use std::time::Instant;
use tokio::process::Command;

pub async fn botinfo(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let Some(guild_id) = interaction.guild_id else {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Command can only be used in a server"),
            )
            .await?;
        return Ok(());
    };

    let bot = ctx.cache.current_user().clone();
    let member = ctx.http.get_member(guild_id, bot.id).await?;
    let user = ctx.http.get_user(bot.id).await?;
    let num_guilds = ctx.cache.guilds().len();
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

    let discord_join = NaiveDateTime::from_timestamp_opt(user.created_at().unix_timestamp(), 0)
        .ok_or_else(|| {
            CommandError::from(format!(
                "Invalid discord join timestamp: {}",
                user.created_at().unix_timestamp()
            ))
        })?;
    let server_join = if let Some(joined_at) = member.joined_at {
        Some(
            NaiveDateTime::from_timestamp_opt(joined_at.unix_timestamp(), 0).ok_or_else(|| {
                CommandError::from(format!(
                    "Invalid server join timestamp: {}",
                    joined_at.unix_timestamp()
                ))
            })?,
        )
    } else {
        None
    };

    let mut embed = CreateEmbed::new()
        .title(member.nick.as_ref().unwrap_or(&bot.name))
        .thumbnail(member.face())
        .timestamp(serenity::model::timestamp::Timestamp::now())
        .field(
            "Joined Discord",
            discord_join.format("%b %e, %Y").to_string(),
            true,
        )
        .field(
            "Joined Server",
            if let Some(joined_at) = server_join {
                joined_at.format("%b %e, %Y").to_string()
            } else {
                String::from("`Unknown`")
            },
            true,
        )
        .field("Member of", format!("{num_guilds} servers"), true)
        .field("Host Uptime", server_uptime, true)
        .field("Bot Uptime", bot_uptime.join("\n"), true);
    if member.nick.is_some() {
        embed = embed.description(&bot.name);
    }
    if let Some(banner) = user.banner_url() {
        embed = embed.image(banner);
    }
    if let Some(color) = user.accent_colour {
        embed = embed.color(color);
    } else if let Some(member_color) = member.colour(&ctx.cache) {
        embed = embed.color(member_color);
    }

    interaction
        .edit_response(&ctx.http, EditInteractionResponse::new().embed(embed))
        .await?;

    Ok(())
}
