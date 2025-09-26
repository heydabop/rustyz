use crate::event::report_interaction_error;
use crate::{commands, twitch};

use num_format::{Locale, ToFormattedString};
use serenity::all::UserId;
use serenity::builder::CreateMessage;
use serenity::client::Context;
use serenity::model::{
    channel::Message,
    event::MessageUpdateEvent,
    id::{ChannelId, MessageId},
};
use sqlx::types::Decimal;
use sqlx::{Pool, Postgres};
use tracing::error;

use super::Handler;

pub async fn create(handler: &Handler, ctx: Context, msg: Message) {
    {
        #[allow(clippy::panic)]
        if let Err(e) = sqlx::query!(
            r#"
INSERT INTO message(discord_id, author_id, channel_id, guild_id, content)
VALUES ($1, $2, $3, $4, $5)"#,
            Decimal::from(msg.id.get()),
            i64::from(msg.author.id),
            i64::from(msg.channel_id),
            msg.guild_id.map(i64::from),
            msg.content
        )
        .execute(&handler.db)
        .await
        {
            error!(%e, "error inserting message into db");
        }
    }
    if let Some(caps) = handler.vote_regex.captures(&msg.content)
        && let Ok(user_id) = caps[1].parse::<u64>().map(UserId::new)
    {
        let is_upvote = &caps[2] == "++";
        if let Some(guild_id) = msg.guild_id {
            match commands::vote::process_vote(&ctx, is_upvote, msg.author.id, guild_id, user_id)
                .await
            {
                Ok(Some(reply)) => {
                    if let Err(e) = msg.reply(&ctx, reply).await {
                        error!(%e, "unable to reply to message");
                    }
                }
                Err(e) => {
                    error!(%e, "unable to process vote message");
                    report_interaction_error(
                        &ctx,
                        format!("error running vote from message: `{e}`",),
                    )
                    .await;
                }
                _ => {}
            }
        } else if let Err(e) = msg.reply(&ctx, "Votes can only be done in a server").await {
            error!(%e, "unable to reply to message");
        }
    }
    if let Some(caps) = handler.twitch_regex.captures(&msg.content)
        && !handler.twitch_clip_regex.is_match(&msg.content)
        && let Some(channel_match) = caps.get(2)
    {
        let channel_name = channel_match.as_str();
        let (access_token, client_id) = match twitch::get_access_token(&ctx).await {
            Ok(a) => a,
            Err(e) => {
                error!(%e, "error getting twitch auth");
                return;
            }
        };
        match twitch::get_stream_info(&access_token, &client_id, channel_name).await {
            Ok(s) => {
                if let Some(stream) = s
                    && let Err(e) = msg
                        .channel_id
                        .send_message(
                            ctx,
                            CreateMessage::new().content(format!(
                                "{} playing {}\n{}\n{} viewers",
                                stream.user_name,
                                stream.game_name,
                                stream.title,
                                stream.viewer_count.to_formatted_string(&Locale::en)
                            )),
                        )
                        .await
                {
                    error!(%e, "error sending twitch message");
                }
            }
            Err(e) => error!(%e, "error getting twitch stream info"),
        }
    }
}

pub async fn delete(db: &Pool<Postgres>, channel_id: ChannelId, message_id: MessageId) {
    #[allow(clippy::panic)]
    if let Err(e) = sqlx::query!(
        r#"DELETE FROM message WHERE channel_id = $1 AND discord_id = $2"#,
        i64::from(channel_id),
        Decimal::from(message_id.get())
    )
    .execute(db)
    .await
    {
        error!(%e, "error deleting message from db");
    }
}

pub async fn delete_bulk(db: &Pool<Postgres>, channel_id: ChannelId, message_ids: Vec<MessageId>) {
    let decimal_message_ids: Vec<Decimal> = message_ids
        .into_iter()
        .map(|m_id| Decimal::from(m_id.get()))
        .collect();
    #[allow(clippy::panic)]
    if let Err(e) = sqlx::query!(
        r#"DELETE FROM message WHERE channel_id = $1 AND discord_id = ANY($2)"#,
        i64::from(channel_id),
        &decimal_message_ids
    )
    .execute(db)
    .await
    {
        error!(%e, "error bulk deleting messages from db");
    }
}

pub async fn update(db: &Pool<Postgres>, update: MessageUpdateEvent) {
    let content: String = if let Some(c) = update.content {
        c
    } else {
        return;
    };
    #[allow(clippy::panic)]
    if let Err(e) = sqlx::query!(
        r#"UPDATE message SET content = $1 WHERE channel_id = $2 AND discord_id = $3"#,
        content,
        i64::from(update.channel_id),
        Decimal::from(update.id.get())
    )
    .execute(db)
    .await
    {
        error!(%e, "error editing message in db");
    }
}
