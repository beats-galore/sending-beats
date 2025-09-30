pub mod analyzer;
pub mod compressor;
pub mod custom_effects_chain;
pub mod default_effects_chain;
pub mod equalizer;
pub mod filter;
pub mod limiter;

pub use analyzer::{AudioAnalyzer, PeakDetector, RmsDetector, SpectrumAnalyzer};
pub use compressor::Compressor;
pub use custom_effects_chain::CustomAudioEffectsChain;
pub use default_effects_chain::DefaultAudioEffectsChain;
pub use equalizer::{EQBand, ThreeBandEqualizer};
pub use filter::BiquadFilter;
pub use limiter::Limiter;

/// Audio stability constants for denormal protection
const DENORMAL_THRESHOLD: f32 = 1e-15;
const MIN_DB: f32 = -100.0;
const MAX_DB: f32 = 40.0;
const MIN_LOG_INPUT: f32 = 1e-10;

/// **BASS POPPING FIX**: More aggressive denormal protection for filter stability
#[inline]
fn flush_denormal(x: f32) -> f32 {
    let abs_x = x.abs();
    if abs_x < DENORMAL_THRESHOLD || !x.is_finite() {
        0.0
    } else if abs_x > 100.0 {
        // Clamp extreme values that could cause instability
        if x > 0.0 {
            100.0
        } else {
            -100.0
        }
    } else {
        x
    }
}

/// Safe logarithm with denormal protection
#[inline]
fn safe_log10(x: f32) -> f32 {
    if x > MIN_LOG_INPUT {
        x.log10()
    } else {
        MIN_LOG_INPUT.log10()
    }
}

/// Safe dB conversion with clamping
#[inline]
fn safe_db_to_linear(db: f32) -> f32 {
    let clamped_db = db.clamp(MIN_DB, MAX_DB);
    10.0_f32.powf(clamped_db / 20.0)
}

/// Clamp and validate floating point values
#[inline]
fn validate_float(x: f32) -> f32 {
    if x.is_finite() {
        flush_denormal(x)
    } else {
        0.0
    }
}
