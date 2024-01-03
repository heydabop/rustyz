use crate::error::CommandResult;
use crate::model::DB;
use crate::util;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::id::UserId;

pub async fn topcommand(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
) -> CommandResult {
    let Some(command) = interaction
        .data
        .options
        .first()
        .and_then(|o| {
            o.resolved.as_ref().map(|r| {
                if let CommandDataOptionValue::String(s) = r {
                    Some(s)
                } else {
                    None
                }
            })
        })
        .unwrap_or(None)
    else {
        return Ok(());
    };

    let Some(guild_id) = interaction.guild_id else {
        return Ok(());
    };

    let members = util::collect_members_guild_id(ctx, guild_id).await?;

    let rows = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        let db = data.get::<DB>().unwrap();
        #[allow(clippy::panic)]
        sqlx::query!(
            r#"
SELECT author_id, count(author_id) AS num_messages
FROM message
WHERE content LIKE $1
AND channel_id = $2
GROUP BY author_id
ORDER BY count(author_id) DESC
LIMIT 10"#,
            format!("/{command}%"),
            i64::try_from(interaction.channel_id.0)?
        )
        .fetch_all(db)
        .await?
    };

    let mut lines = Vec::with_capacity(10_usize);

    for row in &rows {
        let user_id = UserId(u64::try_from(row.author_id)?);
        let num_messages = row.num_messages.unwrap_or(0);
        let username = util::get_username_userid(&ctx.http, &members, user_id).await;
        lines.push(format!("{username} \u{2014} {num_messages}\n"));
    }

    interaction
        .edit_original_interaction_response(&ctx.http, |response| {
            response.content(format!("usage of `{command}`\n{}", lines.concat()))
        })
        .await?;

    Ok(())
}
