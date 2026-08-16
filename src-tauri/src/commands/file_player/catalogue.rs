// Queues as things in their own right, apart from whichever patch is loaded
//
// The running-player commands next door are about one queue that is patched in
// and playing. These are about the collection: what queues exist, what is in
// them, what they have played, and which of them this patch has on its canvas.

use tauri::State;

use crate::db::{FilePlayerStore, FilePlayerTargetService};
use crate::entities::{file_player, file_player_play, file_player_track};
use crate::AudioState;

use super::active_configuration;

/// How much of a queue's play log is worth reading at once
///
/// Bounded because a queue running ads every break builds a long one, and
/// nobody scrolls a year of them — the recent end is the part anybody asks
/// about.
const PLAY_LOG_LIMIT: u64 = 200;

/// Every queue in the studio
#[tauri::command]
pub async fn list_queues(
    audio_state: State<'_, AudioState>,
) -> Result<Vec<file_player::Model>, String> {
    FilePlayerStore::list(audio_state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())
}

/// What is in a queue, in the order it plays
///
/// Read from the database rather than from the running player, so a queue can
/// be looked at and edited without being patched into anything.
#[tauri::command]
pub async fn queue_tracks(
    audio_state: State<'_, AudioState>,
    player_id: String,
) -> Result<Vec<file_player_track::Model>, String> {
    FilePlayerStore::tracks(audio_state.database.sea_orm(), &player_id)
        .await
        .map_err(|e| e.to_string())
}

/// What a queue has played, most recent first
#[tauri::command]
pub async fn queue_plays(
    audio_state: State<'_, AudioState>,
    player_id: String,
) -> Result<Vec<file_player_play::Model>, String> {
    FilePlayerStore::plays(audio_state.database.sea_orm(), &player_id, PLAY_LOG_LIMIT)
        .await
        .map_err(|e| e.to_string())
}

/// Forget what a queue has played, leaving the queue itself alone
#[tauri::command]
pub async fn clear_queue_plays(
    audio_state: State<'_, AudioState>,
    player_id: String,
) -> Result<(), String> {
    FilePlayerStore::clear_plays(audio_state.database.sea_orm(), &player_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_queue(
    audio_state: State<'_, AudioState>,
    player_id: String,
    name: String,
) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("A queue needs a name".to_string());
    }

    FilePlayerStore::rename(audio_state.database.sea_orm(), &player_id, trimmed)
        .await
        .map_err(|e| e.to_string())
}

/// The queues this patch has on its canvas
#[tauri::command]
pub async fn list_queue_targets(audio_state: State<'_, AudioState>) -> Result<Vec<String>, String> {
    let configuration_id = active_configuration(&audio_state).await?;

    FilePlayerTargetService::list_for_configuration(
        audio_state.database.sea_orm(),
        &configuration_id,
    )
    .await
    .map_err(|e| e.to_string())
}

/// Put a queue on this patch so it can be patched into a channel
#[tauri::command]
pub async fn add_queue_target(
    audio_state: State<'_, AudioState>,
    player_id: String,
) -> Result<(), String> {
    let configuration_id = active_configuration(&audio_state).await?;

    FilePlayerTargetService::add(
        audio_state.database.sea_orm(),
        &configuration_id,
        &player_id,
    )
    .await
    .map_err(|e| e.to_string())
}

/// Take a queue off this patch. The queue and everything in it stay.
#[tauri::command]
pub async fn remove_queue_target(
    audio_state: State<'_, AudioState>,
    player_id: String,
) -> Result<(), String> {
    let configuration_id = active_configuration(&audio_state).await?;

    FilePlayerTargetService::remove(
        audio_state.database.sea_orm(),
        &configuration_id,
        &player_id,
    )
    .await
    .map_err(|e| e.to_string())
}
