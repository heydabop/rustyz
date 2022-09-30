use crate::error::CommandResult;
use crate::model::DB;
use crate::util;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::id::UserId;

// Replies to msg with the top users in channel sorted by most messages sent
// Allows a single optional arg of how many users to list, defaults to 5
pub async fn top(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let guild_id = match interaction.guild_id {
        Some(g) => g,
        None => return Ok(()),
    };

    let members = util::collect_members_guild_id(ctx, guild_id).await?;
    let limit: i64 = interaction
        .data
        .options
        .get(0)
        .and_then(|o| {
            o.resolved.as_ref().map(|r| {
                if let CommandDataOptionValue::Integer(l) = r {
                    *l
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
SELECT author_id, count(author_id) AS num_messages
FROM message
WHERE channel_id = $1
AND content NOT LIKE '/%'
GROUP BY author_id
ORDER BY count(author_id) DESC
LIMIT $2"#,
            interaction.channel_id.0 as i64,
            limit
        )
        .fetch_all(db)
        .await?
    };

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mut lines = Vec::with_capacity(limit as usize);

    for row in &rows {
        #[allow(clippy::cast_sign_loss)]
        let user_id = UserId(row.author_id as u64);
        let username = util::get_username_userid(&ctx.http, &members, user_id).await;
        lines.push(format!(
            "{} \u{2014} {}\n",
            username,
            row.num_messages.unwrap_or(0)
        ));
    }

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(lines.concat()))
        .await?;

    Ok(())
}
