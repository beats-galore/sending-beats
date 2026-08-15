// Patchbay colour commands
//
// Every input and destination carries a colour, and a tile saying where a signal
// goes is painted with the colour of the thing it refers to. The palette itself
// lives in the interface — this only remembers what was chosen.

use std::collections::HashMap;
use tauri::State;

use crate::db::{AudioMixerConfigurationService, PatchColorService};
use crate::log_command;
use crate::AudioState;
use colored::*;

/// The active session, or an error saying there is nothing to store against
async fn active_configuration(state: &State<'_, AudioState>) -> Result<String, String> {
    AudioMixerConfigurationService::get_active_session(state.database.sea_orm())
        .await
        .map_err(|e| e.to_string())?
        .map(|session| session.id)
        .ok_or_else(|| "No active session to hold patch colours".to_string())
}

/// Every colour this session has assigned, keyed by what it colours
///
/// Keys are the patchbay's own: `ch:<n>` for an input strip, `out:<identifier>`
/// for a hardware destination, `stream` and `rec` for the broadcast and tape.
/// Anything missing has never been assigned one and should be given an unused
/// colour from the palette.
#[tauri::command]
pub async fn list_patch_colors(
    state: State<'_, AudioState>,
) -> Result<HashMap<String, String>, String> {
    let Some(session) =
        AudioMixerConfigurationService::get_active_session(state.database.sea_orm())
            .await
            .map_err(|e| e.to_string())?
    else {
        return Ok(HashMap::new());
    };

    PatchColorService::list_for_configuration(state.database.sea_orm(), &session.id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_patch_color(
    target_key: String,
    color: String,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    log_command!("set_patch_color", "{} to {}", target_key, color);

    let configuration_id = active_configuration(&state).await?;

    PatchColorService::set(
        state.database.sea_orm(),
        &configuration_id,
        &target_key,
        &color,
    )
    .await
    .map_err(|e| e.to_string())
}

/// Forget something's colour, so it is given a fresh one next time it appears
#[tauri::command]
pub async fn clear_patch_color(
    target_key: String,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    log_command!("clear_patch_color", "{}", target_key);

    let configuration_id = active_configuration(&state).await?;

    PatchColorService::clear(state.database.sea_orm(), &configuration_id, &target_key)
        .await
        .map_err(|e| e.to_string())
}
