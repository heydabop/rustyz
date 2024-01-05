use crate::error::CommandResult;
use crate::util;
use serenity::all::{CommandDataOptionValue, CommandInteraction};
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;
use serenity::model::id::UserId;

// Replies with the username or nickname of the supplied user ID
// Takes a single required argument of a user ID
pub async fn whois(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let Some(guild_id) = interaction.guild_id else {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Command can only be used in a server"),
            )
            .await?;
        return Ok(());
    };

    let user_id = if let CommandDataOptionValue::String(u) = &interaction.data.options[0].value {
        if let Ok(id) = u.parse() {
            UserId::new(id)
        } else {
            interaction
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content("Invalid User ID"),
                )
                .await?;
            return Ok(());
        }
    } else {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Invalid User ID"),
            )
            .await?;
        return Ok(());
    };

    let members = util::collect_members_guild_id(ctx, guild_id).await?;

    let username = util::get_username_userid(&ctx.http, &members, user_id).await;

    interaction
        .edit_response(&ctx.http, EditInteractionResponse::new().content(username))
        .await?;

    Ok(())
}
