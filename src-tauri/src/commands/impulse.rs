// Putting a station on air over Impulse, and taking it off again.
//
// The half of this that touches the mixer is identical to Icecast's: ask the
// audio manager for an output worker under the station's id, and it hands back a
// ring buffer consumer of the mix. Everything downstream of that consumer is
// different, and lives in `audio::broadcasting::impulse`.
//
// The output worker is still created through `AudioCommand::StartIcecast`. That
// command means "give me a consumer for a cast stream" rather than anything
// Icecast-specific — it reads nothing from the config but the sample rate and
// the channel count — and the device id it derives is what the patchbay already
// routes to. Introducing a second name for the same thing would split routing
// in two for no gain.

use tauri::State;

use crate::audio::broadcasting::{get_impulse_service, ImpulseConfig, StreamingServiceConfig};
use crate::audio::mixer::stream_management::AudioCommand;
use crate::AudioState;

/// The mixer's half of the arrangement: sample rate and channels, nothing else
///
/// The audio manager takes a `StreamingServiceConfig` because that is what
/// Icecast handed it first. None of the connection fields are read, and passing
/// the defaults for them says so more clearly than inventing plausible values.
fn output_worker_config(config: &ImpulseConfig) -> StreamingServiceConfig {
    let mut worker = StreamingServiceConfig::default();
    worker.audio_format.sample_rate = config.sample_rate;
    worker.audio_format.channels = config.channels;
    worker.audio_format.bitrate = config.bitrate_kbps;
    worker
}

/// Ask the audio manager to stop feeding this stream
///
/// Used both when coming off air and when going on air failed partway, where
/// leaving the worker running would keep a slice of the mix going to nothing.
async fn release_output_worker(audio_state: &State<'_, AudioState>, stream_id: &str) {
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    let command = AudioCommand::StopIcecast {
        stream_id: stream_id.to_string(),
        response_tx,
    };

    if audio_state.audio_command_tx.send(command).await.is_err() {
        tracing::warn!(
            "⚠️ Could not ask the audio manager to release '{}'",
            stream_id
        );
        return;
    }

    match response_rx.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!("⚠️ Releasing '{}' failed: {}", stream_id, e),
        Err(e) => tracing::warn!("⚠️ No answer releasing '{}': {}", stream_id, e),
    }
}

/// Go on air, sending the mix to the ingest worker as segments
pub(crate) async fn start_impulse_with_id(
    audio_state: &State<'_, AudioState>,
    stream_id: String,
    config: ImpulseConfig,
) -> Result<String, String> {
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    let command = AudioCommand::StartIcecast {
        stream_id: stream_id.clone(),
        config: output_worker_config(&config),
        response_tx,
    };

    if let Err(e) = audio_state.audio_command_tx.send(command).await {
        return Err(format!("Could not reach the audio manager: {}", e));
    }

    let consumer = match response_rx.await {
        Ok(Ok(consumer)) => consumer,
        Ok(Err(e)) => return Err(format!("Could not tap the mix: {}", e)),
        Err(e) => return Err(format!("The audio manager did not answer: {}", e)),
    };

    let slug = config.station_slug.clone();

    match get_impulse_service().await.start(config, consumer).await {
        Ok(()) => Ok(format!("Broadcasting to '{}'", slug)),
        Err(e) => {
            // The transmitter never started, so the output worker is feeding a
            // ring buffer nothing is reading.
            release_output_worker(audio_state, &stream_id).await;
            Err(format!("Could not go on air: {}", e))
        }
    }
}

/// Come off air: finish the queue, sign off, and release the mixer's tap
pub(crate) async fn stop_impulse(
    audio_state: &State<'_, AudioState>,
    stream_id: &str,
) -> Result<(), String> {
    // The transmitter first. It has segments still queued and a sign-off to
    // send, and both need the mix's tail rather than a tap pulled out from
    // under them.
    if let Err(e) = get_impulse_service().await.stop().await {
        tracing::warn!("⚠️ Stopping the Impulse transmitter reported: {}", e);
    }

    release_output_worker(audio_state, stream_id).await;
    Ok(())
}
