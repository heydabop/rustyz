use chrono::naive::NaiveDateTime;
use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};

pub async fn userinfo(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let user = match interaction.data.options.get(0).and_then(|o| {
        o.resolved.as_ref().and_then(|r| {
            if let CommandDataOptionValue::User(u, _) = r {
                Some(u)
            } else {
                None
            }
        })
    }) {
        Some(u) => u,
        None => {
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content("Unable to find user")
                })
                .await?;
            return Ok(());
        }
    };

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

    let member = ctx.http.get_member(guild_id.0, user.id.0).await?;
    let yes = "\u{2705}";
    let no = "\u{274C}";

    interaction
        .edit_original_interaction_response(&ctx.http, |r| {
            r.embed(|e| {
                e.title(member.nick.as_ref().unwrap_or(&user.tag()))
                    .thumbnail(member.face())
                    .field("Bot?", if user.bot { yes } else { no }, true)
                    .field(
                        "Boosting Server?",
                        if let Some(since) = member.premium_since {
                            NaiveDateTime::from_timestamp(since.unix_timestamp(), 0)
                                .format("%b %e, %Y")
                                .to_string()
                        } else {
                            no.to_string()
                        },
                        true,
                    )
                    .field("\u{200B}", "\u{200B}", false)
                    .field(
                        "Joined Discord",
                        NaiveDateTime::from_timestamp(user.created_at().unix_timestamp(), 0)
                            .format("%b %e, %Y")
                            .to_string(),
                        true,
                    )
                    .field(
                        "Joined Server",
                        if let Some(joined_at) = member.joined_at {
                            NaiveDateTime::from_timestamp(joined_at.unix_timestamp(), 0)
                                .format("%b %e, %Y")
                                .to_string()
                        } else {
                            String::from("`Unknown`")
                        },
                        true,
                    );
                if member.nick.is_some() {
                    e.description(user.tag());
                }
                if let Some(banner) = &user.banner {
                    e.image(banner);
                }
                if let Some(color) = user.accent_colour {
                    e.color(color);
                }
                e
            })
        })
        .await?;

    Ok(())
}
