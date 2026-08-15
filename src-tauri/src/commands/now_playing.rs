use crate::audio::now_playing::{
    fetch_now_playing, NowPlayingError, NowPlayingTrack, NowPlayingWatcher, SupportedPlayer,
};
use crate::log_command;
use crate::AudioState;
use colored::Colorize;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::Mutex as AsyncMutex;

pub struct NowPlayingState {
    pub watcher: Arc<AsyncMutex<NowPlayingWatcher>>,
}

impl NowPlayingState {
    pub fn new() -> Self {
        Self {
            watcher: Arc::new(AsyncMutex::new(NowPlayingWatcher::new())),
        }
    }
}

impl Default for NowPlayingState {
    fn default() -> Self {
        Self::new()
    }
}

/// Read one player's current track immediately.
#[tauri::command]
pub async fn get_now_playing(bundle_id: String) -> Result<Option<NowPlayingTrack>, String> {
    log_command!("get_now_playing", "bundle_id={}", bundle_id);

    let player = resolve_player(&bundle_id)?;
    fetch_now_playing(player).await.map_err(|e| e.to_string())
}

/// Start polling for track changes, emitting `now-playing-changed` whenever one
/// lands. The watcher reads the input configuration on every tick, so it only
/// queries a player while that player is configured as an input.
#[tauri::command]
pub async fn start_now_playing_watch(
    app: AppHandle,
    audio_state: State<'_, AudioState>,
    now_playing_state: State<'_, NowPlayingState>,
) -> Result<(), String> {
    log_command!("start_now_playing_watch");

    now_playing_state
        .watcher
        .lock()
        .await
        .start(app, audio_state.database.clone());

    Ok(())
}

#[tauri::command]
pub async fn stop_now_playing_watch(
    now_playing_state: State<'_, NowPlayingState>,
) -> Result<(), String> {
    log_command!("stop_now_playing_watch");

    now_playing_state.watcher.lock().await.stop();

    Ok(())
}

#[tauri::command]
pub async fn is_now_playing_watch_running(
    now_playing_state: State<'_, NowPlayingState>,
) -> Result<bool, String> {
    Ok(now_playing_state.watcher.lock().await.is_running())
}

fn resolve_player(bundle_id: &str) -> Result<SupportedPlayer, String> {
    SupportedPlayer::from_bundle_id(bundle_id).ok_or_else(|| {
        NowPlayingError::UnsupportedPlayer {
            bundle_id: bundle_id.to_string(),
        }
        .to_string()
    })
}
