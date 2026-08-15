// The places this studio broadcasts to
//
// A station is stored once and chosen, rather than typed in before every show.
// Passwords are never sent back out: the interface is told whether one is set,
// which is all a password field needs to draw itself.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::{cast_secrets, CastConfigurationDraft, CastConfigurationService};
use crate::entities::cast_configuration;
use crate::AudioState;

/// A station as the interface sees it: the row, plus whether it can connect
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CastConfigurationView {
    #[serde(flatten)]
    pub configuration: cast_configuration::Model,
    /// Whether a password is in the keychain, without saying what it is
    pub has_password: bool,
}

/// What the interface can set on a station
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CastConfigurationInput {
    pub name: String,
    pub server_host: String,
    pub server_port: i32,
    pub mount_point: String,
    pub username: String,
    #[serde(default)]
    pub stream_name: String,
    #[serde(default)]
    pub stream_description: String,
    #[serde(default)]
    pub stream_genre: String,
    #[serde(default)]
    pub stream_url: String,
    #[serde(default)]
    pub is_public: bool,
    pub audio_format: String,
    pub bitrate_kbps: i32,
    #[serde(default)]
    pub variable_bitrate: bool,
    pub vbr_quality: i32,
}

impl From<CastConfigurationInput> for CastConfigurationDraft {
    fn from(input: CastConfigurationInput) -> Self {
        Self {
            name: input.name,
            server_host: input.server_host,
            server_port: input.server_port,
            mount_point: input.mount_point,
            username: input.username,
            stream_name: input.stream_name,
            stream_description: input.stream_description,
            stream_genre: input.stream_genre,
            stream_url: input.stream_url,
            is_public: input.is_public,
            audio_format: input.audio_format,
            bitrate_kbps: input.bitrate_kbps,
            variable_bitrate: input.variable_bitrate,
            vbr_quality: input.vbr_quality,
        }
    }
}

fn view(configuration: cast_configuration::Model) -> CastConfigurationView {
    let has_password = cast_secrets::has_password(&configuration.id);
    CastConfigurationView {
        configuration,
        has_password,
    }
}

#[tauri::command]
pub async fn list_cast_configurations(
    state: State<'_, AudioState>,
) -> Result<Vec<CastConfigurationView>, String> {
    let rows = CastConfigurationService::list(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(view).collect())
}

#[tauri::command]
pub async fn create_cast_configuration(
    state: State<'_, AudioState>,
    input: CastConfigurationInput,
) -> Result<CastConfigurationView, String> {
    let created = CastConfigurationService::create(state.database.sea_orm(), input.into())
        .await
        .map_err(|e| e.to_string())?;

    Ok(view(created))
}

#[tauri::command]
pub async fn update_cast_configuration(
    state: State<'_, AudioState>,
    id: String,
    input: CastConfigurationInput,
) -> Result<CastConfigurationView, String> {
    let updated = CastConfigurationService::update(state.database.sea_orm(), &id, input.into())
        .await
        .map_err(|e| e.to_string())?;

    Ok(view(updated))
}

#[tauri::command]
pub async fn delete_cast_configuration(
    state: State<'_, AudioState>,
    id: String,
) -> Result<(), String> {
    CastConfigurationService::remove(state.database.sea_orm(), &id)
        .await
        .map_err(|e| e.to_string())
}

/// Store a station's password, or clear it with an empty string
///
/// Kept apart from updating the rest so the interface never has to hold a
/// password it is not changing, and so saving a station's details does not need
/// one at all.
#[tauri::command]
pub async fn set_cast_configuration_password(id: String, password: String) -> Result<bool, String> {
    cast_secrets::set_password(&id, &password).map_err(|e| e.to_string())?;
    Ok(cast_secrets::has_password(&id))
}

/// Put a stored station on air
///
/// Where going live actually happens now. The transmitter's fields used to be
/// sent with the request, except the interface never sent them — the settings
/// on screen and the ones the engine used had no connection at all.
///
/// The password is read from the keychain here rather than being carried
/// through the interface, so the only place it is ever in memory is the moment
/// it is handed to the encoder.
///
/// The stream is registered under the station's own key, which is what makes it
/// something the mixer can be routed to across going off air and back on.
#[tauri::command]
pub async fn start_cast(state: State<'_, AudioState>, id: String) -> Result<String, String> {
    let station = CastConfigurationService::get(state.database.sea_orm(), &id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("No cast configuration '{}'", id))?;

    let password = cast_secrets::password(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!(
                "'{}' has no password stored. Set one before going live.",
                station.name
            )
        })?;

    let config = crate::audio::broadcasting::StreamingServiceConfig {
        server_host: station.server_host.clone(),
        server_port: station.server_port as u16,
        mount_point: station.mount_point.clone(),
        password,
        stream_name: station.stream_name.clone(),
        stream_description: station.stream_description.clone(),
        stream_genre: station.stream_genre.clone(),
        stream_url: station.stream_url.clone(),
        is_public: station.is_public,
        selected_bitrate: station.bitrate_kbps as u32,
        enable_variable_bitrate: station.variable_bitrate,
        vbr_quality: station.vbr_quality as u8,
        ..Default::default()
    };

    crate::commands::icecast::start_icecast_with_id(&state, id, config).await
}
