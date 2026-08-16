// Listener counts for whatever is currently on air.
//
// What is left of a generation of streaming commands that could never run. The
// file declared its own `StreamState` while Tauri managed the different type in
// `lib.rs`, and Tauri resolves managed state by type — so every command here
// asked for something that was never registered and returned an error without
// touching a stream. Six of them were called by nothing at all. The seventh was
// polled by the interface every fifteen seconds and had never once succeeded,
// which is why listener counts have never displayed.
//
// The six are gone. This one is served from the stack that actually broadcasts.

use tauri::State;

use crate::AudioState;

/// Current and peak listeners, from the server's own admin endpoint
///
/// Errors when nothing is on air, which the interface reads as "no counts to
/// show" rather than a failure — a station that publishes no stats and a
/// station nobody is listening to should not look the same as zero listeners.
#[tauri::command]
pub async fn get_listener_stats(_audio_state: State<'_, AudioState>) -> Result<(u32, u32), String> {
    let service = crate::audio::broadcasting::utils::get_streaming_service().await;

    service.listener_stats().await.map_err(|e| e.to_string())
}
