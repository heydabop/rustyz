use crate::error::CommandResult;
use crate::model::GuildVoiceLocks;
use rand::{thread_rng, Rng};
use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use songbird::tracks::{PlayMode, TrackError};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

// Joins the same voice channel as the invoking user and plays audio
pub async fn asuh(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let Some(guild_id) = interaction.guild_id else {
        return Ok(());
    };

    let Some(guild) = ctx.cache.guild(guild_id) else {
        return Err(format!("Unable to find guild {guild_id}").into());
    };

    let Some(voice_channel_id) = guild
        .voice_states
        .get(&interaction.user.id)
        .and_then(|voice_state| voice_state.channel_id)
    else {
        interaction
            .edit_original_interaction_response(&ctx.http, |response| {
                response.content("Command can only be used if you're in a voice channel")
            })
            .await?;
        return Ok(());
    };

    let track_num = {
        let mut rng = thread_rng();
        rng.gen_range(0..=76)
    };
    let track = match songbird::ffmpeg(format!("suh/suh{track_num}.mp3")).await {
        Ok(t) => t,
        Err(e) => {
            return Err(format!("Unable to play mp3: {e}").into());
        }
    };

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
            .or_insert_with(|| Arc::new(Mutex::new(())));
        lock.clone()
    };
    let _voice_lock = voice_lock.lock().await;

    let handler = manager.join(guild_id, voice_channel_id).await;

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content("\u{1F50A}"))
        .await?;

    {
        let mut call = handler.0.lock().await;
        let audio_handle = call.play_only_source(track);
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            match audio_handle.get_info().await {
                Ok(info) => {
                    if info.playing == PlayMode::Stop || info.playing == PlayMode::End {
                        break;
                    }
                }
                Err(e) => {
                    if let TrackError::Finished = e {
                        break;
                    }
                    return Err(format!("Unexpected error during playback: {e}").into());
                }
            }
        }
    }

    if let Err(e) = manager.remove(guild_id).await {
        return Err(format!("Unable to leave after playback: {e}").into());
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    interaction
        .delete_original_interaction_response(&ctx.http)
        .await?;

    Ok(())
}
