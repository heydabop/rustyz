use crate::LastCommandMessages;
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::channel::Message;

// Attempts to delete the last command and response (and this command message) from the author in this channel
#[command]
#[only_in(guilds)]
pub async fn delete(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    let last_messages = {
        ctx.data
            .read()
            .await
            .get::<LastCommandMessages>()
            .unwrap()
            .clone()
    };

    let message_ids = {
        let last_messages = last_messages.read().await;
        let message_ids = last_messages.get(&(msg.channel_id, msg.author.id));
        if let Some(message_ids) = message_ids {
            *message_ids
        } else {
            return Ok(());
        }
    };

    for message_id in message_ids {
        std::mem::drop(
            ctx.http
                .delete_message(msg.channel_id.0, message_id.0)
                .await,
        );
    }
    std::mem::drop(msg.delete(ctx).await);

    Ok(())
}
