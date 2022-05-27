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

pub async fn topcommand(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let command = match interaction
        .data
        .options
        .get(0)
        .and_then(|o| {
            o.resolved.as_ref().map(|r| {
                if let ApplicationCommandInteractionDataOptionValue::String(s) = r {
                    Some(s)
                } else {
                    None
                }
            })
        })
        .unwrap_or(None) {
            Some(s) => s,
            None => return Ok(()),
        };

    let guild_id = match interaction.guild_id {
        Some(g) => g,
        None => return Ok(()),
    };
    let members = util::collect_members_guild_id(ctx, guild_id).await?;

    let rows = {
        let data = ctx.data.read().await;
        let db = data.get::<OldDB>().unwrap();
        sqlx::query(
            r#"
SELECT author_id, count(author_id) AS num_messages
FROM message
WHERE content LIKE $1
AND chan_id = $2
GROUP BY author_id
ORDER BY count(author_id) DESC
LIMIT 10"#,
        )
        .bind(format!("/{}%", command))
        .bind(Decimal::from(interaction.channel_id.0))
        .fetch_all(&*db)
        .await?
    };

    let mut lines = Vec::with_capacity(10_usize);

    for row in &rows {
        let user_id = UserId(row.get::<Decimal, _>(0).to_u64().unwrap());
        let num_messages: i64 = row.get(1);
        let username = util::get_username_userid(&ctx.http, &members, user_id).await;
        lines.push(format!("{} \u{2014} {}\n", username, num_messages));
    }

    interaction
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| message.content(format!("usage of `{}`\n{}", command, lines.concat())))
        })
        .await?;

    Ok(())
}
