use crate::{AudioConfigFactory, AudioState, MixerConfig};
use std::sync::Arc;
use tauri::State;
use tracing::{error, info};

#[tauri::command]
pub fn get_dj_mixer_config() -> MixerConfig {
    AudioConfigFactory::create_dj_config()
}

#[tauri::command]
pub async fn update_master_gain(gain: f32, state: State<'_, AudioState>) -> Result<(), String> {
    info!("🎚️ UPDATE_MASTER_GAIN: Setting master gain to {}", gain);

    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .audio_command_tx
        .send(
            crate::audio::mixer::stream_management::AudioCommand::UpdateMasterGain {
                gain,
                response_tx: tx,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    rx.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    Ok(())
}
