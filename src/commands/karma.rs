use crate::error::CommandResult;
use crate::model::OldDB;
use crate::util;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::id::UserId;
use sqlx::Row;

// Replies with the top users in guild sorted by highest karma (vote count)
// Allows a single optional arg of how many users to list, defaults to 5
pub async fn karma(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let Some(guild_id) = interaction.guild_id else {
        return Ok(());
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
        let db = data.get::<OldDB>().unwrap();
        sqlx::query(
            r#"
SELECT user_id, karma
FROM user_karma
WHERE guild_id = $1
ORDER BY karma DESC
LIMIT $2"#,
        )
        .bind(guild_id.to_string())
        .bind(limit)
        .fetch_all(db)
        .await?
    };

    let mut lines = Vec::with_capacity(usize::try_from(limit)?);

    for row in &rows {
        let user_id = UserId(row.get::<String, _>(0).parse::<u64>()?);
        let karma: i32 = row.get(1);
        let username = util::get_username_userid(&ctx.http, &members, user_id).await;
        lines.push(format!("{username} \u{2014} {karma}\n"));
    }

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(lines.concat()))
        .await?;

    Ok(())
}
