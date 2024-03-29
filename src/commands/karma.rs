use crate::error::CommandResult;
use crate::model::DB;
use crate::util;
use serenity::all::{CommandDataOptionValue, CommandInteraction};
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;
use serenity::model::id::UserId;

// Replies with the top users in guild sorted by highest karma (vote count)
// Allows a single optional arg of how many users to list, defaults to 5
pub async fn karma(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let Some(guild_id) = interaction.guild_id else {
        return Ok(());
    };
    let members = util::collect_members_guild_id(ctx, guild_id).await?;
    let limit: i64 = interaction.data.options.first().map_or(5, |o| {
        if let CommandDataOptionValue::Integer(l) = o.value {
            l
        } else {
            5
        }
    });

    let rows = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        let db = data.get::<DB>().unwrap();
        #[allow(clippy::panic)]
        sqlx::query!(
            r"
SELECT user_id, karma
FROM user_karma
WHERE guild_id = $1
ORDER BY karma DESC
LIMIT $2",
            i64::from(guild_id),
            limit
        )
        .fetch_all(db)
        .await?
    };

    let mut lines = Vec::with_capacity(usize::try_from(limit)?);

    for row in &rows {
        let user_id = row.user_id;
        let karma: i32 = row.karma;
        #[allow(clippy::cast_sign_loss)]
        let username =
            util::get_username_userid(&ctx.http, &members, UserId::new(user_id as u64)).await;
        lines.push(format!("{username} \u{2014} {karma}\n"));
    }

    interaction
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content(lines.concat()),
        )
        .await?;

    Ok(())
}
