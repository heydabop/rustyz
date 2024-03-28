use crate::error::{CommandError, CommandResult};
use crate::model::DB;
use chrono::DateTime;
use num_format::{Locale, ToFormattedString};
use serenity::all::{CommandDataOptionValue, CommandInteraction};
use serenity::builder::{CreateEmbed, EditInteractionResponse};
use serenity::client::Context;

pub async fn userinfo(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let Some(user) = interaction.data.options.first().and_then(|o| {
        if let CommandDataOptionValue::User(u) = o.value {
            Some(u)
        } else {
            None
        }
    }) else {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Unable to find user"),
            )
            .await?;
        return Ok(());
    };

    let Some(guild_id) = interaction.guild_id else {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Command can only be used in a server"),
            )
            .await?;
        return Ok(());
    };

    let member = ctx.http.get_member(guild_id, user).await?;
    // fully populate user
    let user = ctx.http.get_user(user).await?;
    let yes = "\u{2705}";
    let no = "\u{274C}";
    let date_format_str = "%b %e, %Y";

    let db = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<DB>().unwrap().clone()
    };
    #[allow(clippy::panic)]
    let guild_messages: i64 = sqlx::query!(
        r#"
SELECT count(id)
FROM message
WHERE guild_id = $1
AND author_id = $2"#,
        i64::try_from(guild_id)?,
        i64::try_from(user.id)?
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
        i64::try_from(interaction.channel_id)?,
        i64::try_from(user.id)?,
    )
    .fetch_one(&db)
    .await?
    .count
    .unwrap_or(0);
    let karma: i32 = {
        #[allow(clippy::panic)]
        let row = sqlx::query!(
            r"
SELECT karma
FROM user_karma
WHERE guild_id = $1
AND user_id = $2",
            i64::from(guild_id),
            i64::from(user.id)
        )
        .fetch_optional(&db)
        .await?;
        if let Some(r) = row {
            r.karma
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
        i64::try_from(guild_id)?,
        i64::try_from(user.id)?
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
            DateTime::from_timestamp(since.unix_timestamp(), 0).ok_or_else(|| {
                CommandError::from(format!(
                    "Invalid boost timestamp: {}",
                    since.unix_timestamp()
                ))
            })?,
        )
    } else {
        None
    };
    let discord_join =
        DateTime::from_timestamp(user.created_at().unix_timestamp(), 0).ok_or_else(|| {
            CommandError::from(format!(
                "Invalid discord join timestamp: {}",
                user.created_at().unix_timestamp()
            ))
        })?;
    let server_join = if let Some(joined_at) = member.joined_at {
        Some(
            DateTime::from_timestamp(joined_at.unix_timestamp(), 0).ok_or_else(|| {
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
        .title(member.nick.as_ref().unwrap_or(&user.name))
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
        embed = embed.description(&user.name);
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
