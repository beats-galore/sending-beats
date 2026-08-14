use crate::db::seaorm_services::AudioMixerConfigurationService;
use crate::{AudioConfigFactory, AudioState, MixerConfig};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, Set};
use std::sync::Arc;
use tauri::State;
use tracing::{error, info};

/// The mixer layout, with whatever names the active session has given its channels
#[tauri::command]
pub async fn get_dj_mixer_config(state: State<'_, AudioState>) -> Result<MixerConfig, String> {
    let mut config = AudioConfigFactory::create_dj_config();

    let Some(session) =
        AudioMixerConfigurationService::get_active_session(state.database.sea_orm())
            .await
            .map_err(|e| e.to_string())?
    else {
        return Ok(config);
    };

    let named = crate::entities::mixer_channel::Entity::find()
        .filter(crate::entities::mixer_channel::Column::ConfigurationId.eq(&session.id))
        .all(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?;

    for stored in named {
        if let Some(channel) = config
            .channels
            .iter_mut()
            .find(|channel| channel.id as i32 == stored.channel_number)
        {
            channel.name = stored.name;
        }
    }

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
