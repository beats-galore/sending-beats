use crate::audio::mixer::latency_probe::LatencySnapshot;
use crate::db::seaorm_services::AudioMixerConfigurationService;
use crate::{AudioConfigFactory, AudioState, MixerConfig};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, Set};
use tauri::State;
use tracing::info;

/// The mixer layout for the active session
///
/// One channel per input the session has patched, in channel order. A channel
/// exists because something is patched into it — there is no fixed number of
/// strips waiting to be filled, which is what the configurations model replaced.
/// `mixer_channels` holds only the names, since a channel's device is deleted
/// and recreated every time its source is switched.
#[tauri::command]
pub async fn get_dj_mixer_config(state: State<'_, AudioState>) -> Result<MixerConfig, String> {
    let mut config = AudioConfigFactory::create_dj_config();
    config.channels.clear();

    let Some(session) =
        AudioMixerConfigurationService::get_active_session(state.database.sea_orm())
            .await
            .map_err(|e| e.to_string())?
    else {
        return Ok(config);
    };

    let mut patched = crate::entities::configured_audio_device::Entity::find()
        .filter(crate::entities::configured_audio_device::Column::ConfigurationId.eq(&session.id))
        .filter(crate::entities::configured_audio_device::Column::IsInput.eq(true))
        .all(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?;

    patched.sort_by_key(|device| device.channel_number);

    let named = crate::entities::mixer_channel::Entity::find()
        .filter(crate::entities::mixer_channel::Column::ConfigurationId.eq(&session.id))
        .all(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?;

    config.channels = patched
        .into_iter()
        .map(|device| crate::audio::types::AudioChannel {
            id: device.channel_number as u32,
            name: named
                .iter()
                .find(|stored| stored.channel_number == device.channel_number)
                .map(|stored| stored.name.clone())
                .unwrap_or_default(),
            ..Default::default()
        })
        .collect();

    Ok(config)
}

/// Give a channel a name, or clear it back to the device-derived fallback
#[tauri::command]
pub async fn rename_mixer_channel(
    state: State<'_, AudioState>,
    channel_number: i32,
    name: String,
) -> Result<(), String> {
    let session = AudioMixerConfigurationService::get_active_session(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No active session to name a channel in".to_string())?;

    let existing = crate::entities::mixer_channel::Entity::find()
        .filter(crate::entities::mixer_channel::Column::ConfigurationId.eq(&session.id))
        .filter(crate::entities::mixer_channel::Column::ChannelNumber.eq(channel_number))
        .one(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?;

    let trimmed = name.trim().to_string();
    let now = chrono::Utc::now();

    // An empty name is a removal, not a stored blank — the channel goes back to
    // showing whatever is patched into it.
    if trimmed.is_empty() {
        if let Some(model) = existing {
            model
                .delete(state.database.sea_orm())
                .await
                .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    match existing {
        Some(model) => {
            let mut active: crate::entities::mixer_channel::ActiveModel = model.into();
            active.name = Set(trimmed);
            active.updated_at = Set(now);
            active.update(state.database.sea_orm()).await
        }
        None => {
            crate::entities::mixer_channel::ActiveModel {
                id: Set(uuid::Uuid::new_v4().to_string()),
                configuration_id: Set(session.id.clone()),
                channel_number: Set(channel_number),
                name: Set(trimmed),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(state.database.sea_orm())
            .await
        }
    }
    .map_err(|e| e.to_string())?;

    Ok(())
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

/// What the running pipeline is actually costing, stage by stage
#[tauri::command]
pub async fn get_pipeline_latency(state: State<'_, AudioState>) -> Result<LatencySnapshot, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .audio_command_tx
        .send(
            crate::audio::mixer::stream_management::AudioCommand::GetLatencySnapshot {
                response_tx: tx,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    rx.await.map_err(|e| e.to_string())
}
