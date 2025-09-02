use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag to control audio debug logging
pub static AUDIO_DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Set audio debug logging on/off
pub fn set_audio_debug(enabled: bool) {
    AUDIO_DEBUG_ENABLED.store(enabled, Ordering::Relaxed);
    println!("🔧 Audio debug logging {}", if enabled { "ENABLED" } else { "DISABLED" });
}

/// Check if audio debug logging is enabled
pub fn is_audio_debug_enabled() -> bool {
    AUDIO_DEBUG_ENABLED.load(Ordering::Relaxed)
}

/// Audio debug macro - only prints if audio debug is enabled
#[macro_export]
macro_rules! audio_debug {
    ($($arg:tt)*) => {
        if $crate::log::AUDIO_DEBUG_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
            println!($($arg)*);
        }
    };
}