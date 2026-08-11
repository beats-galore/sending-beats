use crate::log_command;
use colored::*;
use tracing::info;

/// Shut the application down
///
/// Used by the restart prompt shown after the virtual audio driver is installed:
/// coreaudiod restarts during installation, which invalidates this process's
/// Core Audio client, so the driver only becomes usable on the next launch.
#[tauri::command]
pub async fn quit_application(app_handle: tauri::AppHandle) -> Result<(), String> {
    log_command!("quit_application");

    info!("{} Shutting down at user request", "APP_QUIT".bright_cyan());

    app_handle.exit(0);

    Ok(())
}
