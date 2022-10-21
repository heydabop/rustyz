use crate::twitch;

use num_format::{Locale, ToFormattedString};
use serenity::client::Context;
use serenity::model::{
    channel::Message,
    event::MessageUpdateEvent,
    id::{ChannelId, MessageId},
};
use sqlx::types::Decimal;
use sqlx::{Pool, Postgres};
use tracing::error;

pub async fn create(ctx: Context, db: &Pool<Postgres>, twitch_regex: &regex::Regex, msg: Message) {
    {
        let author_id = match i64::try_from(msg.author.id.0) {
            Ok(a) => a,
            Err(e) => {
                error!(%e, "unable to fit author id in i64");
                return;
            }
        };
        let channel_id = match i64::try_from(msg.channel_id.0) {
            Ok(c) => c,
            Err(e) => {
                error!(%e, "unable to fit channel id in i64");
                return;
            }
        };
        let guild_id = if let Some(g) = msg.guild_id {
            match i64::try_from(g.0) {
                Ok(g) => Some(g),
                Err(e) => {
                    error!(%e, "unable to fit guild id in i64");
                    return;
                }
            }
        } else {
            None
        };

        #[allow(clippy::panic)]
        if let Err(e) = sqlx::query!(
            r#"
INSERT INTO message(discord_id, author_id, channel_id, guild_id, content)
VALUES ($1, $2, $3, $4, $5)"#,
            Decimal::from(msg.id.0),
            author_id,
            channel_id,
            guild_id,
            msg.content
        )
        .execute(db)
        .await
        {
            error!(%e, "error inserting message into db");
        }
    }
    if let Some(caps) = twitch_regex.captures(&msg.content) {
        if let Some(channel_match) = caps.get(2) {
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
                    if let Some(stream) = s {
                        if let Err(e) = msg
                            .channel_id
                            .send_message(ctx, |m| {
                                m.content(format!(
                                    "{} playing {}\n{}\n{} viewers",
                                    stream.user_name,
                                    stream.game_name,
                                    stream.title,
                                    stream.viewer_count.to_formatted_string(&Locale::en)
                                ))
                            })
                            .await
                        {
                            error!(%e, "error sending twitch message");
                        }
                    }
                }
                Err(e) => error!(%e, "error getting twitch stream info"),
            }
        }
    }
}

pub async fn delete(db: &Pool<Postgres>, channel_id: ChannelId, message_id: MessageId) {
    let channel_id = match i64::try_from(channel_id.0) {
        Ok(c) => c,
        Err(e) => {
            error!(%e, "unable to fit channel id in i64");
            return;
        }
    };
    #[allow(clippy::panic)]
    if let Err(e) = sqlx::query!(
        r#"DELETE FROM message WHERE channel_id = $1 AND discord_id = $2"#,
        channel_id,
        Decimal::from(message_id.0)
    )
    .execute(db)
    .await
    {
        error!(%e, "error deleting message from db");
    }
}

pub async fn delete_bulk(db: &Pool<Postgres>, channel_id: ChannelId, message_ids: Vec<MessageId>) {
    let channel_id = match i64::try_from(channel_id.0) {
        Ok(c) => c,
        Err(e) => {
            error!(%e, "unable to fit channel id in i64");
            return;
        }
    };
    let decimal_message_ids: Vec<Decimal> = message_ids
        .into_iter()
        .map(|m| Decimal::from(m.0))
        .collect();
    #[allow(clippy::panic)]
    if let Err(e) = sqlx::query!(
        r#"DELETE FROM message WHERE channel_id = $1 AND discord_id = ANY($2)"#,
        channel_id,
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
    let channel_id = match i64::try_from(update.channel_id.0) {
        Ok(c) => c,
        Err(e) => {
            error!(%e, "unable to fit channel id in i64");
            return;
        }
    };
    #[allow(clippy::panic)]
    if let Err(e) = sqlx::query!(
        r#"UPDATE message SET content = $1 WHERE channel_id = $2 AND discord_id = $3"#,
        content,
        channel_id,
        Decimal::from(update.id.0)
    )
    .execute(db)
    .await
    {
        error!(%e, "error editing message in db");
    }
}
