pub mod app;
pub mod application_audio;
pub mod audio_devices;
pub mod audio_effects;
pub mod audio_effects_default;
pub mod buses;
pub mod cast_configurations;
pub mod configurations;
pub mod debug;
pub mod device_attachment;
pub mod file_player;
pub mod icecast;
pub mod mixer;
pub mod now_playing;
pub mod patch_colors;
pub mod patch_layouts;
pub mod process_metrics;
pub mod recording;
pub mod streaming;
pub mod system_audio;
pub mod vu_channels;

/// Log a command invocation at the API boundary
/// This helps track which frontend calls are triggering backend operations
#[macro_export]
macro_rules! log_command {
    ($cmd:expr) => {
        tracing::info!(
            "🔷 {} {}",
            "API_COMMAND".on_white().purple(),
            $cmd
        );
    };
    ($cmd:expr, $($arg:tt)*) => {
        tracing::info!(
            "🔷 {} {}: {}",
            "API_COMMAND".on_white().white(),
            $cmd,
            format!($($arg)*)
        );
    };
}
