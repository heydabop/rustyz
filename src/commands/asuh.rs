use crate::error::CommandResult;
use crate::model::GuildVoiceLocks;
use rand::{thread_rng, Rng};
use serenity::all::CommandInteraction;
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;
use songbird::input::File;
use songbird::tracks::{ControlError, PlayMode, Track};
use std::sync::atomic::{AtomicI16, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout};
use tracing::warn;

const ONE_SECOND: Duration = Duration::from_secs(1);

// Joins the same voice channel as the invoking user and plays audio
pub async fn asuh(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let Some(guild_id) = interaction.guild_id else {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Command can only be used in a server"),
            )
            .await?;
        return Ok(());
    };

    let voice_states = match ctx.cache.guild(guild_id) {
        Some(g) => g.voice_states.clone(),
        None => return Err(format!("Unable to find guild {guild_id}").into()),
    };

    if voice_states
        .get(&interaction.user.id)
        .and_then(|voice_state| voice_state.channel_id)
        .is_none()
    {
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new()
                    .content("Command can only be used if you're in a voice channel"),
            )
            .await?;
        return Ok(());
    };

    let track_num = {
        let mut rng = thread_rng();
        rng.gen_range(0..=76)
    };
    let track = Track::from(File::new(format!("suh/suh{track_num}.mp3")));

    let Some(manager) = songbird::get(ctx).await else {
        return Err("Missing songbird".into());
    };

    let voice_lock = {
        let map_mutex = {
            let data = ctx.data.read().await;
            #[allow(clippy::unwrap_used)]
            data.get::<GuildVoiceLocks>().unwrap().clone()
        };
        let mut voice_locks = map_mutex.lock().await;
        let lock = voice_locks
            .entry(guild_id)
            .or_insert_with(|| Arc::new((Mutex::new(()), AtomicI16::new(0))));
        lock.clone()
    };
    voice_lock.1.fetch_add(1, Ordering::Relaxed); //indicate we're about to be waiting on this lock
    let _voice_mutex = voice_lock.0.lock().await;
    voice_lock.1.fetch_sub(1, Ordering::Relaxed);

    // refresh voice_channel_id
    let voice_states = match ctx.cache.guild(guild_id) {
        Some(g) => g.voice_states.clone(),
        None => return Err(format!("Unable to find guild {guild_id}").into()),
    };
    let Some(voice_channel_id) = voice_states
        .get(&interaction.user.id)
        .and_then(|voice_state| voice_state.channel_id)
    else {
        let leave_res = if manager.get(guild_id).is_some() {
            // check if we need to leave a call now
            if voice_lock.1.load(Ordering::Relaxed) < 1 {
                // only leave the channel if we dont think anyone is waiting on the lock
                if let Err(e) = manager.remove(guild_id).await {
                    Err(format!("Unable to leave after playback: {e}").into())
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            }
        } else {
            Ok(())
        };
        interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new()
                    .content("Command can only be used if you're in a voice channel"),
            )
            .await?;
        return leave_res;
    };

    let handler = manager.join(guild_id, voice_channel_id).await?;

    interaction
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content("\u{1F50A}"),
        )
        .await?;

    let play_result = {
        let mut call = handler.lock().await;
        let audio_handle = call.play_only(track);
        loop {
            sleep(ONE_SECOND).await;
            let Ok(info) = timeout(ONE_SECOND, audio_handle.get_info()).await else {
                // this appears to hapeen when bot is kicked/disconnect from channel
                warn!("get_info took too long");
                break Ok(());
            };
            match info {
                Ok(info) => {
                    if info.playing == PlayMode::Stop || info.playing == PlayMode::End {
                        break Ok(());
                    }
                }
                Err(e) => {
                    if let ControlError::Finished = e {
                        break Ok(());
                    }
                    break Err(format!("Unexpected error during playback: {e}").into());
                }
            }
        }
    };

    sleep(ONE_SECOND).await;

    if voice_lock.1.load(Ordering::Relaxed) < 1 {
        // only leave the channel if we dont think anyone is waiting on the lock
        if let Err(e) = manager.remove(guild_id).await {
            return Err(format!("Unable to leave after playback: {e}").into());
        }
    }

    interaction.delete_response(&ctx.http).await?;

    play_result
}
