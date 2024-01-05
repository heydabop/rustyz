use crate::error::{CommandError, CommandResult};
use crate::model::DB;
use chrono::naive::NaiveDateTime;
use num_format::{Locale, ToFormattedString};
use serenity::all::CommandInteraction;
use serenity::builder::{CreateEmbed, EditInteractionResponse};
use serenity::client::Context;
use serenity::model::prelude::PremiumTier;

pub async fn serverinfo(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let Some(guild_id) = interaction.guild_id else {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Command can only be used in a server"),
            )
            .await?;
        return Ok(());
    };

    let guild = guild_id.to_partial_guild_with_counts(&ctx.http).await?;

    let db = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<DB>().unwrap().clone()
    };

    #[allow(clippy::panic)]
    let num_messages: i64 = sqlx::query!(
        r#"
SELECT count(id)
FROM message
WHERE guild_id = $1"#,
        i64::try_from(guild_id)?
    )
    .fetch_one(&db)
    .await?
    .count
    .unwrap_or(0);

    let created = NaiveDateTime::from_timestamp_opt(guild.id.created_at().unix_timestamp(), 0)
        .ok_or_else(|| {
            CommandError::from(format!(
                "Invalid server creation timestamp: {}",
                guild.id.created_at().unix_timestamp()
            ))
        })?;

    let mut embed = CreateEmbed::new()
        .title(&guild.name)
        .timestamp(serenity::model::timestamp::Timestamp::now())
        .field("Created on", created.format("%b %e, %Y").to_string(), true)
        .field(
            "Boost Tier",
            match guild.premium_tier {
                PremiumTier::Tier0 => "None",
                PremiumTier::Tier1 => "Level 1",
                PremiumTier::Tier2 => "Level 2",
                PremiumTier::Tier3 => "Level 3",
                _ => "?",
            },
            true,
        );
    if let Some(count) = guild.premium_subscription_count {
        embed = embed.field("Boosts", count.to_formatted_string(&Locale::en), true);
    }
    embed = embed
        .field(
            "Messages",
            num_messages.to_formatted_string(&Locale::en),
            true,
        )
        .field(
            "Members",
            if let Some(count) = guild.approximate_member_count {
                count.to_formatted_string(&Locale::en)
            } else {
                "?".to_string()
            },
            true,
        )
        .field(
            "Online Members",
            if let Some(count) = guild.approximate_presence_count {
                count.to_formatted_string(&Locale::en)
            } else {
                "?".to_string()
            },
            true,
        );
    if let Some(description) = &guild.description {
        embed = embed.description(description);
    }
    if let Some(splash) = guild.splash_url() {
        embed = embed.image(splash);
    }
    if let Some(icon) = guild.icon_url() {
        embed = embed.thumbnail(icon);
    }

    interaction
        .edit_response(&ctx.http, EditInteractionResponse::new().embed(embed))
        .await?;

    Ok(())
}
