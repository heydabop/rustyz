use crate::model::OldDB;
use crate::util;
use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::application::interaction::{
    application_command::{ApplicationCommandInteraction, CommandDataOptionValue},
    InteractionResponseType,
};
use serenity::model::id::UserId;
use sqlx::Row;

// Replies with the top users in guild sorted by highest karma (vote count)
// Allows a single optional arg of how many users to list, defaults to 5
pub async fn karma(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    interaction
        .create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
        })
        .await?;

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
SELECT user_id, karma
FROM user_karma
WHERE guild_id = $1
ORDER BY karma DESC
LIMIT $2"#,
        )
        .bind(guild_id.to_string())
        .bind(limit)
        .fetch_all(&*db)
        .await?
    };

    let mut lines = Vec::with_capacity(limit as usize);

    for row in &rows {
        let user_id = UserId(row.get::<String, _>(0).parse::<u64>().unwrap());
        let karma: i32 = row.get(1);
        let username = util::get_username_userid(&ctx.http, &members, user_id).await;
        lines.push(format!("{} \u{2014} {}\n", username, karma));
    }

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(lines.concat()))
        .await?;

    Ok(())
}
