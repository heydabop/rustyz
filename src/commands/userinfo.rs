use crate::error::{CommandError, CommandResult};
use crate::model::{OldDB, DB};
use chrono::naive::NaiveDateTime;
use num_format::{Locale, ToFormattedString};
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use sqlx::Row;

pub async fn userinfo(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let Some(user) = interaction.data.options.get(0).and_then(|o| {
        o.resolved.as_ref().and_then(|r| {
            if let CommandDataOptionValue::User(u, _) = r {
                Some(u)
            } else {
                None
            }
        })
    }) else {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content("Unable to find user")
            })
            .await?;
        return Ok(());
    };

    let Some(guild_id) = interaction.guild_id else {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content("Command can only be used in a server")
            })
            .await?;
        return Ok(());
    };

    let member = ctx.http.get_member(guild_id.0, user.id.0).await?;
    // fully populate user
    let user = ctx.http.get_user(user.id.0).await?;
    let yes = "\u{2705}";
    let no = "\u{274C}";
    let date_format_str = "%b %e, %Y";

    let (old_db, db) = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        (
            data.get::<OldDB>().unwrap().clone(),
            data.get::<DB>().unwrap().clone(),
        )
    };
    #[allow(clippy::panic)]
    let guild_messages: i64 = sqlx::query!(
        r#"
SELECT count(id)
FROM message
WHERE guild_id = $1
AND author_id = $2"#,
        i64::try_from(guild_id.0)?,
        i64::try_from(user.id.0)?
    )
    .fetch_one(&db)
    .await?
    .count
    .unwrap_or(0);
    #[allow(clippy::panic)]
    let channel_messages: i64 = sqlx::query!(
        r#"
SELECT count(id)
FROM message
WHERE channel_id = $1
AND author_id = $2"#,
        i64::try_from(interaction.channel_id.0)?,
        i64::try_from(user.id.0)?,
    )
    .fetch_one(&db)
    .await?
    .count
    .unwrap_or(0);
    let karma: i32 = {
        let row = sqlx::query(
            r"
SELECT karma
FROM user_karma
WHERE guild_id = $1
AND user_id = $2",
        )
        .bind(guild_id.0.to_string())
        .bind(user.id.0.to_string())
        .fetch_optional(&old_db)
        .await?;
        if let Some(r) = row {
            r.get(0)
        } else {
            0
        }
    };
    #[allow(clippy::panic)]
    let first_message_date: String = match sqlx::query!(
        r#"
SELECT min(create_date) as min_date
FROM message
WHERE guild_id = $1
AND author_id = $2"#,
        i64::try_from(guild_id.0)?,
        i64::try_from(user.id.0)?
    )
    .fetch_one(&db)
    .await?
    .min_date
    {
        Some(date) => date.format(date_format_str).to_string(),
        None => String::from("Unavailable"),
    };

    let boost_timestamp = if let Some(since) = member.premium_since {
        Some(
            NaiveDateTime::from_timestamp_opt(since.unix_timestamp(), 0).ok_or_else(|| {
                CommandError::from(format!(
                    "Invalid boost timestamp: {}",
                    since.unix_timestamp()
                ))
            })?,
        )
    } else {
        None
    };
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

    interaction
        .edit_original_interaction_response(&ctx.http, |r| {
            r.embed(|e| {
                e.title(member.nick.as_ref().unwrap_or(&user.name))
                    .thumbnail(member.face())
                    .timestamp(serenity::model::timestamp::Timestamp::now())
                    .field("Bot?", if user.bot { yes } else { no }, true)
                    .field(
                        "Boosting Server?",
                        if let Some(boost_timestamp) = boost_timestamp {
                            format!("Since {}", boost_timestamp.format(date_format_str))
                        } else {
                            no.to_string()
                        },
                        true,
                    )
                    .field(
                        "Joined Discord",
                        discord_join.format(date_format_str).to_string(),
                        true,
                    )
                    .field(
                        "Joined Server",
                        if let Some(joined_at) = server_join {
                            joined_at.format(date_format_str).to_string()
                        } else {
                            String::from("`Unknown`")
                        },
                        true,
                    )
                    .field("First Message", first_message_date, true)
                    .field(
                        "Server Messages",
                        guild_messages.to_formatted_string(&Locale::en),
                        true,
                    )
                    .field(
                        "Channel Messages",
                        channel_messages.to_formatted_string(&Locale::en),
                        true,
                    )
                    .field("Karma", karma.to_formatted_string(&Locale::en), true);
                if member.nick.is_some() {
                    e.description(&user.name);
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
