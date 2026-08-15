// Patchbay arrangement commands
//
// The canvas computes where everything goes — sources down the left, mixes down
// the middle, destinations down the right. These remember the places the user
// has since moved things to, so a hand-arranged patch comes back the way it was
// left. A node with nothing stored here is still being placed by the canvas.

use std::collections::HashMap;
use tauri::State;

use crate::db::{AudioMixerConfigurationService, PatchLayoutService, Placement};
use crate::log_command;
use crate::AudioState;
use colored::*;

/// The active session, or an error saying there is nothing to store against
async fn active_configuration(state: &State<'_, AudioState>) -> Result<String, String> {
    AudioMixerConfigurationService::get_active_session(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?
        .map(|session| session.id)
        .ok_or_else(|| "No active session to hold the patch arrangement".to_string())
}

/// Everywhere this session has put something, keyed by what it places
///
/// Keys are the patchbay's own: `ch:<n>` for an input strip, `bus:<id>` for a
/// mix, `out:<identifier>` for a hardware destination, `stream` and `rec` for
/// the broadcast and tape. Anything missing has never been moved and should be
/// stacked into its column.
#[tauri::command]
pub async fn list_patch_layouts(
    state: State<'_, AudioState>,
) -> Result<HashMap<String, Placement>, String> {
    let Some(session) =
        AudioMixerConfigurationService::get_active_session(state.database.sea_orm())
            .await
            .map_err(|e| e.to_string())?
    else {
        return Ok(HashMap::new());
    };

    PatchLayoutService::list_for_configuration(state.database.sea_orm(), &session.id)
        .await
        .map_err(|e| e.to_string())
}

/// Put something somewhere, replacing wherever it was
///
/// The placement is the whole of it rather than a patch over the stored one, so
/// a node that is dragged back to taking its own size sends a null size rather
/// than omitting it.
#[tauri::command]
pub async fn set_patch_layout(
    target_key: String,
    placement: Placement,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    log_command!("set_patch_layout", "{} to {:?}", target_key, placement);

    let configuration_id = active_configuration(&state).await?;

    PatchLayoutService::set(
        state.database.sea_orm(),
        &configuration_id,
        &target_key,
        placement,
    )
    .await
    .map_err(|e| e.to_string())
}

/// Forget where something was put, so the canvas places it again
#[tauri::command]
pub async fn clear_patch_layout(
    target_key: String,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    log_command!("clear_patch_layout", "{}", target_key);

    let configuration_id = active_configuration(&state).await?;

    PatchLayoutService::clear(state.database.sea_orm(), &configuration_id, &target_key)
        .await
        .map_err(|e| e.to_string())
}

/// Forget the whole arrangement, putting every node back in its column
#[tauri::command]
pub async fn clear_patch_layouts(state: State<'_, AudioState>) -> Result<(), String> {
    log_command!("clear_patch_layouts", "tidying the canvas");

    let configuration_id = active_configuration(&state).await?;

    PatchLayoutService::clear_all(state.database.sea_orm(), &configuration_id)
        .await
        .map_err(|e| e.to_string())
}
