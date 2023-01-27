use crate::commands;
use crate::event::report_interaction_error;
use crate::model;

use chrono::prelude::*;
use serenity::client::Context;
use serenity::model::{
    application::interaction::{Interaction, InteractionResponseType},
    channel::MessageFlags,
};
use sqlx::{Pool, Postgres};
use tracing::error;

pub async fn create(ctx: Context, db: &Pool<Postgres>, interaction: Interaction) {
    if let Interaction::ApplicationCommand(command) = interaction {
        if let Err(e) = command
            .create_interaction_response(&ctx.http, |response| {
                response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
            })
            .await
        {
            error!(%e, "Unable to defer response to interaction");
            report_interaction_error(
                &ctx,
                format!("unable to defer response to interaction: `{e}`"),
            )
            .await;
            return;
        }
        crate::event::record_command(db, &command).await;
        if let Err(e) = match command.data.name.as_str() {
            "affixes" => commands::affixes::affixes(&ctx, &command).await,
            "birdtime" => commands::time::time(&ctx, &command, "Europe/Oslo").await,
            "botinfo" => commands::botinfo::botinfo(&ctx, &command).await,
            "forecast" => commands::weather::forecast(&ctx, &command).await,
            "fortune" => commands::fortune::fortune(&ctx, &command).await,
            "invite" => commands::invite::invite(&ctx, &command).await,
            "karma" => commands::karma::karma(&ctx, &command).await,
            "lastseen" => commands::lastseen::lastseen(&ctx, &command).await,
            "lastplayed" => commands::lastplayed::lastplayed(&ctx, &command).await,
            "mirotime" => commands::time::time(&ctx, &command, "Europe/Helsinki").await,
            "nieltime" => commands::time::time(&ctx, &command, "Europe/Stockholm").await,
            "ping" => commands::ping::ping(&ctx, &command).await,
            "playtime" => commands::playtime::playtime(&ctx, &command).await,
            "raiderio" => commands::raiderio::raiderio(&ctx, &command).await,
            "realtime" => commands::time::time(&ctx, &command, "America/Chicago").await,
            "recentplaytime" => commands::playtime::recent_playtime(&ctx, &command).await,
            "roll" => commands::roll::roll(&ctx, &command).await,
            "sebbitime" => commands::time::time(&ctx, &command, "Europe/Copenhagen").await,
            "serverinfo" => commands::serverinfo::serverinfo(&ctx, &command).await,
            "source" => commands::source::source(&ctx, &command).await,
            "tarkov" => commands::tarkov::tarkov(&ctx, &command).await,
            "top" => commands::top::top(&ctx, &command).await,
            "topcommand" => commands::topcommand::topcommand(&ctx, &command).await,
            "toplength" => commands::toplength::toplength(&ctx, &command).await,
            "track" => commands::shipping::track(&ctx, &command).await,
            "userinfo" => commands::userinfo::userinfo(&ctx, &command).await,
            "weather" => commands::weather::weather(&ctx, &command).await,
            "whois" => commands::whois::whois(&ctx, &command).await,
            "zalgo" => commands::zalgo::zalgo(&ctx, &command).await,
            "wolframalpha" => commands::wolframalpha::simple(&ctx, &command).await,
            "wow" => {
                if let Some(subcommand) = command.data.options.get(0) {
                    match subcommand.name.as_str() {
                        "character" => {
                            commands::wow::character(&ctx, &command, &subcommand.options).await
                        }
                        "realm" => commands::wow::realm(&ctx, &command, &subcommand.options).await,
                        "search" => {
                            commands::wow::search(&ctx, &command, &subcommand.options).await
                        }
                        "transmog" => {
                            commands::wow::transmog(&ctx, &command, &subcommand.options).await
                        }
                        _ => Err("Unrecognized wow subcommand".into()),
                    }
                } else {
                    Err("Missing wow subcommand".into())
                }
            }
            _ => {
                error!(command = command.data.name, "Missing command");
                report_interaction_error(&ctx, format!("missing command: {}", command.data.name))
                    .await;
                if let Err(e) = command
                    .edit_original_interaction_response(&ctx.http, |response| {
                        response.content("\u{26A0} `Unknown command`")
                    })
                    .await
                {
                    error!(%e, "Unable to respond to interaction");
                    report_interaction_error(
                        &ctx,
                        format!("unable to respond to interaction: `{e}`"),
                    )
                    .await;
                }
                Ok(())
            }
        } {
            error!(%e, command = command.data.name, "Error running command");
            report_interaction_error(&ctx, format!("error running {}: `{e}`", command.data.name))
                .await;
            if let Err(resp_e) = command
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content(format!("\u{26A0} `Error: {e}`"))
                })
                .await
            {
                error!(e = %resp_e, "Unable to respond to interaction");
                report_interaction_error(
                    &ctx,
                    format!("unable to respond to interaction: `{resp_e}`"),
                )
                .await;
            }
        }
    } else if let Interaction::MessageComponent(interaction) = interaction {
        let fields: Vec<&str> = interaction.data.custom_id.split(':').collect();
        let command = fields[0];
        if command != "playtime" {
            return;
        }
        let prev_next = fields[1];
        let button_id = match fields[2].parse::<i32>() {
            Ok(id) => id,
            Err(e) => {
                error!(error = %e, "error parsing button_id from playtime interaction");
                return;
            }
        };
        let row = {
            let data = ctx.data.read().await;
            #[allow(clippy::unwrap_used)]
            let db = data.get::<model::DB>().unwrap();
            #[allow(clippy::panic)]
            match sqlx::query!(r#"SELECT author_id, user_ids, username, start_date, end_date, start_offset FROM playtime_button WHERE id = $1"#, button_id).fetch_one(db).await {
                Ok(row) => row,
                Err(e) => {
                    error!(error = %e, "error getting playtime interaction buttons");
                    return;
                }
            }
        };

        let user_ids: Vec<i64> = row.user_ids;
        let username: Option<String> = row.username;
        let start_date: Option<DateTime<Utc>> = row.start_date;
        let end_date: DateTime<Utc> = row.end_date;
        let offset: i32 = {
            if prev_next == "prev" {
                (row.start_offset - i32::from(commands::playtime::OFFSET_INC)).max(0)
            } else if prev_next == "next" {
                row.start_offset + i32::from(commands::playtime::OFFSET_INC)
            } else {
                return;
            }
        };

        #[allow(clippy::unwrap_used)] // offset isn't negative
        let new_content = match commands::playtime::gen_playtime_message(
            &ctx,
            &user_ids,
            &username,
            start_date,
            end_date,
            usize::try_from(offset).unwrap(),
        )
        .await
        {
            Ok(c) => c,
            Err(e) => {
                error!(error = %e, "error generating new content for playtime interaction");
                return;
            }
        };
        if let Some(flags) = interaction.message.flags {
            if flags.contains(MessageFlags::EPHEMERAL) {
                return;
            }
        }
        let mut message = interaction.message.clone();
        if let Err(e) = message
            .edit(&ctx, |m| {
                m.content(&new_content);
                m.components(|c| {
                    commands::playtime::create_components(c, offset, &new_content, button_id, true)
                });
                m
            })
            .await
        {
            error!(error = %e, "error updating playtime messge components");
            return;
        }

        if let Err(e) = interaction
            .create_interaction_response(&ctx, |r| {
                r.kind(InteractionResponseType::UpdateMessage);
                r
            })
            .await
        {
            error!(error = %e, "error creating playtime interaction response");
            return;
        }

        {
            let data = ctx.data.read().await;
            #[allow(clippy::unwrap_used)]
            let db = data.get::<model::DB>().unwrap();
            #[allow(clippy::panic)]
            if let Err(e) = sqlx::query!(
                r#"UPDATE playtime_button SET start_offset = $2 WHERE id = $1"#,
                button_id,
                offset
            )
            .execute(db)
            .await
            {
                error!(error = %e, "error updating playtime_button table after interaction");
            }
        }

        // leave buttons disabled for 5 seconds, then send the message again with buttons enabled
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        if let Err(e) = message
            .edit(&ctx, |m| {
                m.components(|c| {
                    commands::playtime::create_components(c, offset, &new_content, button_id, false)
                });
                m
            })
            .await
        {
            error!(error = %e, "error updating playtime messge components");
        }
    }
}
