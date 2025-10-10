pub mod application_audio;
pub mod audio_devices;
pub mod audio_effects;
pub mod audio_effects_default;
pub mod configurations;
pub mod debug;
pub mod file_player;
pub mod icecast;
pub mod mixer;
pub mod recording;
pub mod streaming;
pub mod system_audio;
pub mod vu_channels;

pub use application_audio::*;
pub use audio_devices::*;
pub use audio_effects::*;
pub use audio_effects_default::*;
pub use configurations::*;
pub use debug::*;
pub use file_player::*;
pub use icecast::*;
pub use mixer::*;
pub use recording::*;
pub use streaming::*;
pub use system_audio::*;
pub use vu_channels::*;

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
