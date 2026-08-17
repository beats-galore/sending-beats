// The places this studio broadcasts to
//
// A station is stored once and chosen, rather than typed in before every show.
// Passwords are never sent back out: the interface is told whether one is set,
// which is all a password field needs to draw itself.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::audio::broadcasting::{on_air, CastProtocol, ImpulseConfig, StreamingServiceStatus};
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
    /// `icecast` or `impulse`. Defaulted so a caller written before there was a
    /// choice keeps describing the station it always did.
    #[serde(default = "default_protocol")]
    pub protocol: String,
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
    #[serde(default)]
    pub endpoint_url: Option<String>,
    #[serde(default)]
    pub station_slug: Option<String>,
    #[serde(default = "default_segment_ms")]
    pub segment_ms: i32,
}

fn default_protocol() -> String {
    CastProtocol::Icecast.as_str().to_string()
}

/// Four seconds, which is what the far end is built around
///
/// It is the master latency knob there: the same number sets the target
/// duration, the playlist cache lifetime and the point at which a station is
/// treated as having gone quiet.
fn default_segment_ms() -> i32 {
    4_000
}

impl From<CastConfigurationInput> for CastConfigurationDraft {
    fn from(input: CastConfigurationInput) -> Self {
        Self {
            name: input.name,
            protocol: input.protocol,
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
            endpoint_url: input.endpoint_url,
            station_slug: input.station_slug,
            segment_ms: input.segment_ms,
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

/// Put a stored station on air, by whichever protocol it is configured for
///
/// Where going live actually happens. The transmitter's fields used to be sent
/// with the request, except the interface never sent them — the settings on
/// screen and the ones the engine used had no connection at all.
///
/// The secret is read from the keychain here rather than being carried through
/// the interface, so the only place it is ever in memory is the moment it is
/// handed to the transmitter.
///
/// The stream is registered under the station's own key, which is what makes it
/// something the mixer can be routed to across going off air and back on.
#[tauri::command]
pub async fn start_cast(state: State<'_, AudioState>, id: String) -> Result<String, String> {
    let station = CastConfigurationService::get(state.database.sea_orm(), &id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("No cast configuration '{}'", id))?;

    let protocol = CastProtocol::from_stored(&station.protocol).map_err(|e| e.to_string())?;

    // One station at a time. Going live over one protocol while another is
    // already transmitting would leave the first with no way to be stopped.
    if let Some(live) = on_air::current() {
        return Err(format!(
            "Already on air as '{}'. Cut that feed before starting another.",
            live.station_id
        ));
    }

    let secret = cast_secrets::password(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| missing_secret(&station.name, protocol))?;

    let started = match protocol {
        CastProtocol::Icecast => start_over_icecast(&state, &station, secret, id.clone()).await,
        CastProtocol::Impulse => start_over_impulse(&state, &station, secret, id.clone()).await,
    }?;

    on_air::set(id, protocol);

    Ok(started)
}

/// What to say when a station has no secret stored
///
/// The two protocols authenticate against different things, and telling someone
/// to set a "password" when the field they are looking at is a token is how you
/// get a support conversation instead of a broadcast.
fn missing_secret(name: &str, protocol: CastProtocol) -> String {
    match protocol {
        CastProtocol::Icecast => format!(
            "'{}' has no password stored. Set one before going live.",
            name
        ),
        CastProtocol::Impulse => format!(
            "'{}' has no ingest token stored. Set one before going live.",
            name
        ),
    }
}

async fn start_over_icecast(
    state: &State<'_, AudioState>,
    station: &cast_configuration::Model,
    password: String,
    stream_id: String,
) -> Result<String, String> {
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

    crate::commands::icecast::start_icecast_with_id(state, stream_id, config).await
}

async fn start_over_impulse(
    state: &State<'_, AudioState>,
    station: &cast_configuration::Model,
    token: String,
    stream_id: String,
) -> Result<String, String> {
    // Both are required and neither has a sensible default: without them there
    // is no address to send to, and a station that goes "live" to nowhere is
    // worse than one that refuses to start.
    let endpoint_url = station
        .endpoint_url
        .clone()
        .filter(|url| !url.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "'{}' has no ingest endpoint. Set one before going live.",
                station.name
            )
        })?;

    let station_slug = station
        .station_slug
        .clone()
        .filter(|slug| !slug.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "'{}' has no station slug, so there is nothing on the other end to send to.",
                station.name
            )
        })?;

    let config = ImpulseConfig {
        endpoint_url,
        station_slug,
        token,
        segment_ms: station.segment_ms.max(1) as u64,
        sample_rate: crate::types::DEFAULT_SAMPLE_RATE,
        channels: 2,
        bitrate_kbps: station.bitrate_kbps as u32,
    };

    crate::commands::impulse::start_impulse_with_id(state, stream_id, config).await
}

/// Come off air, whichever transmitter is running
///
/// Takes no arguments on purpose: there is one broadcast, the backend knows
/// which station and which protocol it is, and asking the interface to remember
/// is how a stop request ends up pointed at a stream that is not the live one.
#[tauri::command]
pub async fn stop_cast(state: State<'_, AudioState>) -> Result<String, String> {
    let Some(live) = on_air::current() else {
        return Ok("Nothing is on air".to_string());
    };

    let result = match live.protocol {
        CastProtocol::Icecast => {
            crate::commands::icecast::stop_icecast_streaming(state, live.station_id.clone())
                .await
                .map(|_| ())
        }
        CastProtocol::Impulse => {
            crate::commands::impulse::stop_impulse(&state, &live.station_id).await
        }
    };

    // Cleared either way. A transmitter that failed to shut down cleanly is
    // still not on air, and leaving the record set would make it impossible to
    // start anything again.
    on_air::clear();
    crate::audio::broadcasting::metadata::clear();

    result.map(|_| "Off air".to_string())
}

/// What is on air, and how it is going
#[tauri::command]
pub async fn get_cast_status() -> Result<StreamingServiceStatus, String> {
    Ok(crate::audio::broadcasting::cast_status().await)
}

/// The stations on the current patch
#[tauri::command]
pub async fn list_cast_targets(state: State<'_, AudioState>) -> Result<Vec<String>, String> {
    let Some(session) =
        crate::db::AudioMixerConfigurationService::get_active_session(state.database.sea_orm())
            .await
            .map_err(|e| e.to_string())?
    else {
        return Ok(Vec::new());
    };

    crate::db::CastTargetService::list_for_configuration(state.database.sea_orm(), &session.id)
        .await
        .map_err(|e| e.to_string())
}

/// Put a station on the current patch, so it can be routed to
#[tauri::command]
pub async fn add_cast_target(
    state: State<'_, AudioState>,
    cast_configuration_id: String,
) -> Result<(), String> {
    let session_id = active_session_id(&state).await?;

    crate::db::CastTargetService::add(
        state.database.sea_orm(),
        &session_id,
        &cast_configuration_id,
    )
    .await
    .map_err(|e| e.to_string())
}

/// Take a station off the current patch
///
/// Its routing is left where it is, so putting it back finds the sources it had
/// rather than a destination that has to be wired up again.
#[tauri::command]
pub async fn remove_cast_target(
    state: State<'_, AudioState>,
    cast_configuration_id: String,
) -> Result<(), String> {
    let session_id = active_session_id(&state).await?;

    crate::db::CastTargetService::remove(
        state.database.sea_orm(),
        &session_id,
        &cast_configuration_id,
    )
    .await
    .map_err(|e| e.to_string())
}

/// The patch a cast target belongs to, or an error saying there is none
async fn active_session_id(state: &State<'_, AudioState>) -> Result<String, String> {
    crate::db::AudioMixerConfigurationService::get_active_session(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?
        .map(|session| session.id)
        .ok_or_else(|| "No active session to hold the cast destination".to_string())
}
