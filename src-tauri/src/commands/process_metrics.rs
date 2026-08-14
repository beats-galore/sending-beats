use tauri::State;

use crate::process_metrics::{ProcessMetrics, ProcessMonitor};

/// State wrapper so the monitor lives across calls
///
/// CPU is a difference between two refreshes, so a monitor created per call
/// would have nothing to compare against and always report zero.
pub struct ProcessMonitorState(pub ProcessMonitor);

/// What this process is currently costing in CPU and memory
#[tauri::command]
pub async fn get_process_metrics(
    state: State<'_, ProcessMonitorState>,
) -> Result<ProcessMetrics, String> {
    Ok(state.0.sample())
}
