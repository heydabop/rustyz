use crate::model::OldDB;
use crate::util;
use num_traits::cast::ToPrimitive;
use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::id::UserId;
use sqlx::types::Decimal;
use sqlx::Row;

// Replies to msg with the top users in channel sorted by most messages sent
// Allows a single optional arg of how many users to list, defaults to 5
pub async fn top(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
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
        let db = data.get::<OldDB>().unwrap();
        sqlx::query(
            r#"
SELECT author_id, count(author_id) AS num_messages
FROM message
WHERE chan_id = $1
AND content NOT LIKE '/%'
GROUP BY author_id
ORDER BY count(author_id) DESC
LIMIT $2"#,
        )
        .bind(Decimal::from(interaction.channel_id.0))
        .bind(limit)
        .fetch_all(&*db)
        .await?
    };

    let mut lines = Vec::with_capacity(limit as usize);

    for row in &rows {
        let user_id = UserId(row.get::<Decimal, _>(0).to_u64().unwrap());
        let num_messages: i64 = row.get(1);
        let username = util::get_username_userid(&ctx.http, &members, user_id).await;
        lines.push(format!("{} \u{2014} {}\n", username, num_messages));
    }

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(lines.concat()))
        .await?;

    Ok(())
}
