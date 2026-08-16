use crate::{
    audio::{FilePlayerConfig, PlaybackAction, PlaybackStatus, PlayerEvent, QueuedTrack},
    db::{AudioMixerConfigurationService, FilePlayerStore, QueuedTrackRow, TRACK_PENDING},
    AudioState,
};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

mod persistence;
use persistence::{queued_track_from_row, spawn_history_writer};

// State for file player service
pub struct FilePlayerState {
    pub service: crate::audio::FilePlayerService,
}

/// The session a player belongs to, or an error saying there is nothing to
/// store it against
async fn active_configuration(state: &State<'_, AudioState>) -> Result<String, String> {
    AudioMixerConfigurationService::get_active_session(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?
        .map(|session| session.id)
        .ok_or_else(|| "No active session to hold the file player".to_string())
}

// File player management commands
#[tauri::command]
pub async fn create_file_player(
    file_player_state: State<'_, FilePlayerState>,
    audio_state: State<'_, AudioState>,
    config: FilePlayerConfig,
) -> Result<String, String> {
    println!("🎵 Creating file player: {}", config.name);

    let configuration_id = active_configuration(&audio_state).await?;

    // Stored first, because its row key is the identity the running player is
    // created under — the other way round would mint one id and then store a
    // different one.
    let stored = FilePlayerStore::create(
        audio_state.database.sea_orm(),
        &configuration_id,
        &config.name,
        config.sample_rate,
        config.channels,
    )
    .await
    .map_err(|e| e.to_string())?;

    let player_id = file_player_state
        .service
        .get_manager()
        .create_player_with_id(stored.id.clone(), config)
        .map_err(|e| e.to_string())?;

    if let Some(device) = file_player_state
        .service
        .get_manager()
        .get_player(&player_id)
    {
        spawn_history_writer(&device.get_player(), Arc::clone(&audio_state.database));
    }

    Ok(player_id)
}

#[tauri::command]
pub async fn remove_file_player(
    file_player_state: State<'_, FilePlayerState>,
    audio_state: State<'_, AudioState>,
    player_id: String,
) -> Result<(), String> {
    println!("🗑️ Removing file player: {}", player_id);

    file_player_state
        .service
        .get_manager()
        .remove_player(&player_id)
        .map_err(|e| e.to_string())?;

    FilePlayerStore::remove(audio_state.database.sea_orm(), &player_id)
        .await
        .map_err(|e| e.to_string())
}

/// Rebuild the session's file players and their queues
///
/// Called once the session's devices are in place, the way bus routing is
/// restored: a player has to exist before a channel patched to it can be
/// attached, and its queue has to be back before it is played.
#[tauri::command]
pub async fn restore_file_players(
    file_player_state: State<'_, FilePlayerState>,
    audio_state: State<'_, AudioState>,
) -> Result<Vec<String>, String> {
    let configuration_id = active_configuration(&audio_state).await?;
    let db = audio_state.database.sea_orm();

    let stored = FilePlayerStore::list_for_configuration(db, &configuration_id)
        .await
        .map_err(|e| e.to_string())?;

    let manager = file_player_state.service.get_manager();
    let mut restored = Vec::new();

    for row in stored {
        // Already running, so its queue is whatever it has played down to
        // rather than what was saved.
        if manager.get_player(&row.id).is_some() {
            restored.push(row.id);
            continue;
        }

        let config = FilePlayerConfig {
            name: row.name.clone(),
            sample_rate: row.sample_rate as u32,
            channels: row.channels as u16,
            auto_play_next: true,
            volume: row.volume,
        };

        manager
            .create_player_with_id(row.id.clone(), config)
            .map_err(|e| e.to_string())?;

        let Some(device) = manager.get_player(&row.id) else {
            continue;
        };
        let player = device.get_player();

        player.set_mode(persistence::repeat_mode_from(&row.repeat_mode), row.shuffle);
        spawn_history_writer(&player, Arc::clone(&audio_state.database));

        let pending = FilePlayerStore::tracks(db, &row.id, TRACK_PENDING)
            .await
            .map_err(|e| e.to_string())?;

        for track in pending {
            player.enqueue(queued_track_from_row(track));
        }

        // After the queue, so the break lands on a track the player has. One
        // pointing at a track that is no longer pending would never fire.
        if let Some(track_id) = row.breakpoint_track_id.clone() {
            if player.get_queue().iter().any(|track| track.id == track_id) {
                player.set_breakpoint(Some(track_id));
            }
        }

        restored.push(row.id);
    }

    println!("🎵 Restored {} file player(s)", restored.len());
    Ok(restored)
}

#[tauri::command]
pub async fn list_file_players(
    file_player_state: State<'_, FilePlayerState>,
) -> Result<Vec<(String, String)>, String> {
    Ok(file_player_state.service.get_manager().list_players())
}

#[tauri::command]
pub async fn get_file_player_devices(
    file_player_state: State<'_, FilePlayerState>,
) -> Result<Vec<crate::AudioDeviceInfo>, String> {
    Ok(file_player_state.service.get_manager().get_devices())
}

// Queue management commands
#[tauri::command]
pub async fn add_track_to_player(
    file_player_state: State<'_, FilePlayerState>,
    audio_state: State<'_, AudioState>,
    player_id: String,
    file_path: String,
) -> Result<String, String> {
    println!("📀 Adding track to player {}: {}", player_id, file_path);

    let path = PathBuf::from(&file_path);
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|e| format!("Could not read '{}': {}", file_path, e))?;

    if !metadata.is_file() {
        return Err(format!("'{}' is not a file", file_path));
    }

    let device = file_player_state
        .service
        .get_manager()
        .get_player(&player_id)
        .ok_or_else(|| format!("No file player '{}'", player_id))?;

    // Read once, here, rather than when the track reaches the decoder: a queue
    // has to show what is in it before it plays, which is the whole point of
    // being able to look at one.
    let probed = {
        let path = path.clone();
        tokio::task::spawn_blocking(move || crate::audio::read_metadata(&path))
            .await
            .map_err(|e| format!("Could not read '{}': {}", file_path, e))?
    };

    // Written down first, so the id in the queue is the id of the row: a track
    // finishing later has to be recordable against something.
    let stored = FilePlayerStore::queue_track(
        audio_state.database.sea_orm(),
        &player_id,
        QueuedTrackRow {
            file_path: &file_path,
            title: probed.title.as_deref(),
            artist: probed.artist.as_deref(),
            album: probed.album.as_deref(),
            duration_ms: probed.duration.map(|value| value.as_millis() as i64),
            file_size: metadata.len() as i64,
        },
    )
    .await
    .map_err(|e| e.to_string())?;

    let track_id = stored.id.clone();
    device.get_player().enqueue(queued_track_from_row(stored));

    Ok(track_id)
}

#[tauri::command]
pub async fn remove_track_from_player(
    file_player_state: State<'_, FilePlayerState>,
    audio_state: State<'_, AudioState>,
    player_id: String,
    track_id: String,
) -> Result<(), String> {
    println!("🗑️ Removing track {} from player {}", track_id, player_id);

    let manager = file_player_state.service.get_manager();

    manager
        .remove_track_from_player(&player_id, &track_id)
        .map_err(|e| e.to_string())?;

    FilePlayerStore::remove_track(audio_state.database.sea_orm(), &track_id)
        .await
        .map_err(|e| e.to_string())?;

    // Removing the track a break was set after removes the break, which the
    // player has already done in memory. Written back here rather than left to
    // the foreign key, which SQLite does not act on unless asked to.
    persist_breakpoint(&manager, &audio_state, &player_id).await
}

/// Write down whatever break the player is now holding
async fn persist_breakpoint(
    manager: &crate::audio::FilePlayerManager,
    audio_state: &State<'_, AudioState>,
    player_id: &str,
) -> Result<(), String> {
    let Some(device) = manager.get_player(player_id) else {
        return Ok(());
    };

    FilePlayerStore::set_breakpoint(
        audio_state.database.sea_orm(),
        player_id,
        device.get_player().breakpoint().as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

/// Move a queued track to a new place in the queue
///
/// The player moves it, then says what order it is now in and that is what gets
/// written down — one answer rather than the same move made twice.
#[tauri::command]
pub async fn move_track_in_player(
    file_player_state: State<'_, FilePlayerState>,
    audio_state: State<'_, AudioState>,
    player_id: String,
    track_id: String,
    to_index: usize,
) -> Result<(), String> {
    let device = file_player_state
        .service
        .get_manager()
        .get_player(&player_id)
        .ok_or_else(|| format!("No file player '{}'", player_id))?;

    let player = device.get_player();
    player
        .move_track(&track_id, to_index)
        .map_err(|e| e.to_string())?;

    let order: Vec<String> = player
        .get_queue()
        .into_iter()
        .map(|track| track.id)
        .collect();

    FilePlayerStore::reorder_queue(audio_state.database.sea_orm(), &player_id, &order)
        .await
        .map_err(|e| e.to_string())
}

/// Pause after a given track, or stop pausing anywhere
///
/// One instruction rather than a mode: it fires once and clears itself, so the
/// queue never shows a break that has already been taken.
#[tauri::command]
pub async fn set_player_breakpoint(
    file_player_state: State<'_, FilePlayerState>,
    audio_state: State<'_, AudioState>,
    player_id: String,
    track_id: Option<String>,
) -> Result<(), String> {
    let device = file_player_state
        .service
        .get_manager()
        .get_player(&player_id)
        .ok_or_else(|| format!("No file player '{}'", player_id))?;

    device.get_player().set_breakpoint(track_id.clone());

    FilePlayerStore::set_breakpoint(
        audio_state.database.sea_orm(),
        &player_id,
        track_id.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_player_queue(
    file_player_state: State<'_, FilePlayerState>,
    player_id: String,
) -> Result<Vec<QueuedTrack>, String> {
    file_player_state
        .service
        .get_manager()
        .get_player_queue(&player_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_player_queue(
    file_player_state: State<'_, FilePlayerState>,
    audio_state: State<'_, AudioState>,
    player_id: String,
) -> Result<(), String> {
    println!("🧹 Clearing queue for player: {}", player_id);

    let manager = file_player_state.service.get_manager();

    manager
        .clear_player_queue(&player_id)
        .map_err(|e| e.to_string())?;

    // Only what was waiting. What the player already played stays, because that
    // is the history rather than the queue.
    FilePlayerStore::clear_queue(audio_state.database.sea_orm(), &player_id)
        .await
        .map_err(|e| e.to_string())?;

    persist_breakpoint(&manager, &audio_state, &player_id).await
}

// Playback control commands
#[tauri::command]
pub async fn control_file_player(
    file_player_state: State<'_, FilePlayerState>,
    audio_state: State<'_, AudioState>,
    player_id: String,
    action: PlaybackAction,
) -> Result<(), String> {
    println!("🎮 Controlling player {}: {:?}", player_id, action);

    file_player_state
        .service
        .get_manager()
        .control_player(&player_id, action)
        .await
        .map_err(|e| e.to_string())?;

    // Volume is the one control that outlives the session, so it is written back
    // rather than left to be set again next launch.
    if let Some(device) = file_player_state
        .service
        .get_manager()
        .get_player(&player_id)
    {
        let status = device.get_player().get_status();
        let _ = FilePlayerStore::update_playback(
            audio_state.database.sea_orm(),
            &player_id,
            status.volume,
            persistence::repeat_mode_name(status.mode.repeat_mode),
            status.mode.shuffle,
        )
        .await;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_player_status(
    file_player_state: State<'_, FilePlayerState>,
    player_id: String,
) -> Result<PlaybackStatus, String> {
    file_player_state
        .service
        .get_manager()
        .get_player_status(&player_id)
        .map_err(|e| e.to_string())
}

/// Ask the system for audio files to queue
///
/// The dialog is the reliable way in. Dragging from Finder gives paths too, but
/// only while the window can be reached with a full hand of files — this works
/// from anywhere, and is the one route that cannot be missed.
///
/// Returns an empty list when the dialog is dismissed, which is a choice rather
/// than a failure and reads better as nothing to add.
#[tauri::command]
pub async fn browse_audio_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    println!("🔍 Opening audio file browser dialog");

    let (tx, rx) = tokio::sync::oneshot::channel();

    // Callback rather than blocking: the dialog runs on the main thread, and a
    // command awaiting it there would hold up the very thread drawing it.
    app.dialog()
        .file()
        .add_filter("Audio", &SUPPORTED_AUDIO_EXTENSIONS)
        .set_title("Add to the queue")
        .pick_files(move |picked| {
            let _ = tx.send(picked);
        });

    let picked = rx
        .await
        .map_err(|_| "The file browser closed without answering".to_string())?;

    Ok(picked
        .unwrap_or_default()
        .into_iter()
        .filter_map(|path| path.into_path().ok())
        .map(|path| path.to_string_lossy().into_owned())
        .collect())
}

/// What the decoder can open
///
/// One list, used by the browser's filter and by the answer given to the
/// interface, so the dialog cannot offer a file the queue would then refuse.
const SUPPORTED_AUDIO_EXTENSIONS: [&str; 6] = ["mp3", "flac", "wav", "ogg", "m4a", "aac"];

#[tauri::command]
pub async fn get_supported_audio_formats() -> Result<Vec<String>, String> {
    Ok(SUPPORTED_AUDIO_EXTENSIONS
        .iter()
        .map(|extension| extension.to_string())
        .collect())
}

#[tauri::command]
pub async fn validate_audio_file(file_path: String) -> Result<bool, String> {
    let path = PathBuf::from(file_path);

    // Check if file exists
    if !path.exists() {
        return Ok(false);
    }

    // Check if it's a file (not directory)
    if !path.is_file() {
        return Ok(false);
    }

    // Check file extension
    if let Some(extension) = path.extension() {
        if let Some(ext_str) = extension.to_str() {
            let supported_formats = vec!["mp3", "flac", "wav", "ogg", "m4a", "aac"];
            return Ok(supported_formats.contains(&ext_str.to_lowercase().as_str()));
        }
    }

    Ok(false)
}
