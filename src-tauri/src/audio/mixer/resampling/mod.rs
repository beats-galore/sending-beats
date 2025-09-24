// Resampling module - Sample rate conversion implementations
//
// Provides multiple sample rate conversion backends:
// - rubato: FFT-based resampler for fixed input sizes
// - samplerate: libsamplerate-based resampler for variable input sizes
// - r8brain: Professional resampler with continuous read pointer philosophy

pub mod r8brain;
pub mod rubato;
pub mod samplerate;

// Re-export common types for convenience
pub use r8brain::*;
pub use rubato::*;
pub use samplerate::*;
