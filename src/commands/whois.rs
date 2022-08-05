use crate::util;
use serenity::client::Context;
use serenity::framework::standard::{CommandError, CommandResult};
use serenity::model::application::interaction::{
    application_command::{ApplicationCommandInteraction, CommandDataOptionValue},
    InteractionResponseType,
};
use serenity::model::id::UserId;

// Replies with the username or nickname of the supplied user ID
// Takes a single required argument of a user ID
pub async fn whois(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    if interaction.guild_id.is_none() {
        return Ok(());
    }

    interaction
        .create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
        })
        .await?;

    let user_id = if let CommandDataOptionValue::String(u) =
        interaction.data.options[0].resolved.as_ref().unwrap()
    {
        if let Ok(id) = u.parse() {
            UserId(id)
        } else {
            return Err(CommandError::from("Invalid user ID"));
        }
    } else {
        return Err(CommandError::from("Invalid user ID"));
    };

    let members = util::collect_members_guild_id(ctx, interaction.guild_id.unwrap()).await?;

    let username = util::get_username_userid(&ctx.http, &members, user_id).await;

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(username))
        .await?;

    Ok(())
}
