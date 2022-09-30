use crate::error::CommandResult;
use crate::model::{OldDB, DB};
use chrono::naive::NaiveDateTime;
use num_format::{Locale, ToFormattedString};
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use sqlx::Row;

pub async fn userinfo(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let user = match interaction.data.options.get(0).and_then(|o| {
        o.resolved.as_ref().and_then(|r| {
            if let CommandDataOptionValue::User(u, _) = r {
                Some(u)
            } else {
                None
            }
        })
    }) {
        Some(u) => u,
        None => {
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content("Unable to find user")
                })
                .await?;
            return Ok(());
        }
    };

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

    let member = ctx.http.get_member(guild_id.0, user.id.0).await?;
    // fully populate user
    let user = ctx.http.get_user(user.id.0).await?;
    let yes = "\u{2705}";
    let no = "\u{274C}";

    let (old_db, db) = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        (
            data.get::<OldDB>().unwrap().clone(),
            data.get::<DB>().unwrap().clone(),
        )
    };
    #[allow(clippy::cast_possible_wrap)]
    let guild_messages: i64 = sqlx::query!(
        r#"
SELECT count(id)
FROM message
WHERE guild_id = $1
AND author_id = $2"#,
        guild_id.0 as i64,
        user.id.0 as i64
    )
    .fetch_one(&db)
    .await?
    .count
    .unwrap_or(0);
    #[allow(clippy::cast_possible_wrap)]
    let channel_messages: i64 = sqlx::query!(
        r#"
SELECT count(id)
FROM message
WHERE channel_id = $1
AND author_id = $2"#,
        interaction.channel_id.0 as i64,
        user.id.0 as i64,
    )
    .fetch_one(&db)
    .await?
    .count
    .unwrap_or(0);
    let karma: i32 = {
        let row = sqlx::query(
            r#"
SELECT karma
FROM user_karma
WHERE guild_id = $1
AND user_id = $2"#,
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

    interaction
        .edit_original_interaction_response(&ctx.http, |r| {
            r.embed(|e| {
                e.title(member.nick.as_ref().unwrap_or(&user.tag()))
                    .thumbnail(member.face())
                    .timestamp(serenity::model::timestamp::Timestamp::now())
                    .field("Bot?", if user.bot { yes } else { no }, true)
                    .field(
                        "Boosting Server?",
                        if let Some(since) = member.premium_since {
                            format!(
                                "Since {}",
                                NaiveDateTime::from_timestamp(since.unix_timestamp(), 0)
                                    .format("%b %e, %Y")
                            )
                        } else {
                            no.to_string()
                        },
                        true,
                    )
                    .field("\u{200B}", "\u{200B}", false)
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
                    .field("\u{200B}", "\u{200B}", false)
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
                    e.description(user.tag());
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
