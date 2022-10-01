use crate::error::CommandResult;
use crate::model::DB;
use chrono::naive::NaiveDateTime;
use num_format::{Locale, ToFormattedString};
use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::PremiumTier;

pub async fn serverinfo(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
) -> CommandResult {
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
        i64::try_from(guild_id.0)?
    )
    .fetch_one(&db)
    .await?
    .count
    .unwrap_or(0);

    interaction
        .edit_original_interaction_response(&ctx.http, |r| {
            r.embed(|e| {
                e.title(&guild.name)
                    .timestamp(serenity::model::timestamp::Timestamp::now())
                    .field(
                        "Created on",
                        NaiveDateTime::from_timestamp(guild.id.created_at().unix_timestamp(), 0)
                            .format("%b %e, %Y")
                            .to_string(),
                        true,
                    )
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
                    )
                    .field(
                        "Boosts",
                        guild
                            .premium_subscription_count
                            .to_formatted_string(&Locale::en),
                        true,
                    )
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
                    e.description(description);
                }
                if let Some(splash) = guild.splash_url() {
                    e.image(splash);
                }
                if let Some(icon) = guild.icon_url() {
                    e.thumbnail(icon);
                }
                e
            })
        })
        .await?;

    Ok(())
}
