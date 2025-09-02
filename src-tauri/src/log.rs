use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DebugLoggingConfig {
    pub audio: bool,
    pub device: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum DebugLoggingCategory {
    Audio,
    Device,
}

/// Global flag to control audio debug logging
pub static AUDIO_DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);
pub static DEVICE_DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Set audio debug logging on/off
pub fn set_debug_levels(details: DebugLoggingConfig) {
    AUDIO_DEBUG_ENABLED.store(details.audio, Ordering::Relaxed);
    DEVICE_DEBUG_ENABLED.store(details.device, Ordering::Relaxed);
    println!("🔧 Audio debug logging {}", if details.audio { "ENABLED" } else { "DISABLED" });
    println!("🔧 Device debug logging {}", if details.device { "ENABLED" } else { "DISABLED" });
}

pub fn get_debug_levels() -> DebugLoggingConfig {
    DebugLoggingConfig {
        audio: AUDIO_DEBUG_ENABLED.load(Ordering::Relaxed),
        device: DEVICE_DEBUG_ENABLED.load(Ordering::Relaxed),
    }
}

/// Check if audio debug logging is enabled
pub fn is_debug_enabled(category: DebugLoggingCategory) -> bool {
    match category {
        DebugLoggingCategory::Audio => AUDIO_DEBUG_ENABLED.load(Ordering::Relaxed),
        DebugLoggingCategory::Device => DEVICE_DEBUG_ENABLED.load(Ordering::Relaxed),
    }
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

#[macro_export]
macro_rules! device_debug {
    ($($arg:tt)*) => {
        if $crate::log::DEVICE_DEBUG_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
            println!($($arg)*);
        }
    };
}
