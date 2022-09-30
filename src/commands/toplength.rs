use crate::error::CommandResult;
use crate::model::DB;
use crate::util;
use num_traits::cast::ToPrimitive;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::id::UserId;
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
                if let CommandDataOptionValue::Integer(l) = r {
                    *l as u32
                } else {
                    5
                }
            })
        })
        .unwrap_or(5);

    let rows = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        let db = data.get::<DB>().unwrap();
        #[allow(clippy::panic, clippy::cast_possible_wrap)]
        sqlx::query!(
            r#"
SELECT author_id, content
FROM message
WHERE channel_id = $1
AND content NOT LIKE '/%'"#,
            interaction.channel_id.0 as i64
        )
        .fetch_all(db)
        .await?
    };

    let mut messages_per_user: HashMap<u64, u64> = HashMap::new();
    let mut words_per_user: HashMap<u64, usize> = HashMap::new();

    for row in &rows {
        let user_id = match row.author_id.to_u64() {
            Some(u) => u,
            None => return Err("unable to convert user id from db".into()),
        };
        let message = match &row.content {
            Some(c) => c,
            None => return Err("missing message content from db".into()),
        };
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
        let words = match words_per_user.get(user_id) {
            Some(w) => w,
            None => return Err("missing wordcount for user".into()),
        };
        let username = util::get_username_userid(&ctx.http, &members, UserId(*user_id)).await;
        #[allow(clippy::cast_precision_loss)]
        if *messages != 0 {
            avg_per_user.push((username, *words as f64 / *messages as f64));
        }
    }
    #[allow(clippy::unwrap_used)]
    avg_per_user.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    avg_per_user.truncate(limit as usize);

    let lines: Vec<String> = avg_per_user
        .iter()
        .map(|u| format!("{} \u{2014} {:.2}\n", u.0, u.1))
        .collect();

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(lines.concat()))
        .await?;

    Ok(())
}
