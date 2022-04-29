use crate::model::OldDB;
use crate::util;
use num_traits::cast::ToPrimitive;
use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::id::UserId;
use serenity::model::interactions::{
    application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue,
    },
    InteractionResponseType,
};
use sqlx::types::Decimal;
use sqlx::Row;
use std::collections::HashMap;

// Replies to msg with users in channel sorted by average length of sent messages
// Allows a single optional arg of how many users to list, defaults to 5
pub async fn toplength(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
) -> CommandResult {
    let guild_id = match interaction.guild_id {
        Some(g) => g,
        None => return Ok(()),
    };
    let members = util::collect_members_guild_id(ctx, guild_id).await?;
    let limit: u32 = interaction
        .data
        .options
        .get(0)
        .and_then(|o| {
            o.resolved.as_ref().map(|r| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                if let ApplicationCommandInteractionDataOptionValue::Integer(l) = r {
                    *l as u32
                } else {
                    5
                }
            })
        })
        .unwrap_or(5);

    let rows = {
        let data = ctx.data.read().await;
        let db = data.get::<OldDB>().unwrap();
        sqlx::query(
            r#"
SELECT author_id, content
FROM message
WHERE chan_id = $1
AND content NOT LIKE '/%'"#,
        )
        .bind(Decimal::from(interaction.channel_id.0))
        .fetch_all(&*db)
        .await?
    };

    let mut messages_per_user: HashMap<u64, u64> = HashMap::new();
    let mut words_per_user: HashMap<u64, usize> = HashMap::new();

    for row in &rows {
        let user_id = row.get::<Decimal, _>(0).to_u64().unwrap();
        let message = row.get::<String, _>(1);
        let num_words = message.split(' ').count();
        if let Some(messages) = messages_per_user.get_mut(&user_id) {
            *messages += 1;
        } else {
            messages_per_user.insert(user_id, 1);
        }
        if let Some(words) = words_per_user.get_mut(&user_id) {
            *words += num_words;
        } else {
            words_per_user.insert(user_id, num_words);
        }
    }

    let mut avg_per_user: Vec<(String, f64)> = vec![];
    for (user_id, messages) in &messages_per_user {
        let words = words_per_user.get(user_id).unwrap();
        let username = util::get_username_userid(&ctx.http, &members, UserId(*user_id)).await;
        #[allow(clippy::cast_precision_loss)]
        avg_per_user.push((username, *words as f64 / *messages as f64));
    }
    avg_per_user.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    avg_per_user.truncate(limit as usize);

    let lines: Vec<String> = avg_per_user
        .iter()
        .map(|u| format!("{} \u{2014} {:.2}\n", u.0, u.1))
        .collect();

    interaction
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| message.content(lines.concat()))
        })
        .await?;

    Ok(())
}
