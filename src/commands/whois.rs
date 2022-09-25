use crate::util;
use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::id::UserId;

// Replies with the username or nickname of the supplied user ID
// Takes a single required argument of a user ID
pub async fn whois(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let guild_id = match interaction.guild_id {
        Some(g) => g,
        None => {
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content("Command can only be used in a server")
                })
                .await?;
            return Ok(());
        }
    };

    let user_id = if let CommandDataOptionValue::String(u) =
        match interaction.data.options[0].resolved.as_ref() {
            Some(u) => u,
            None => return Err("missing user ID".into()),
        } {
        if let Ok(id) = u.parse() {
            UserId(id)
        } else {
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content("Invalid User ID")
                })
                .await?;
            return Ok(());
        }
    } else {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content("Invalid User ID")
            })
            .await?;
        return Ok(());
    };

    let members = util::collect_members_guild_id(ctx, guild_id).await?;

    let username = util::get_username_userid(&ctx.http, &members, user_id).await;

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(username))
        .await?;

    Ok(())
}
