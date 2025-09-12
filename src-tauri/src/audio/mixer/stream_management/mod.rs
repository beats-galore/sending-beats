// Stream management module - Audio stream lifecycle and device coordination
//
// This module has been split into separate files for better organization:
// - audio_input_stream: Input stream handling and effects processing
// - audio_output_stream: Output stream management and SPMC integration
// - isolated_audio_manager: Main audio processing manager with event-driven architecture
// - stream_manager: Hardware stream management for CoreAudio/CPAL integration

pub mod audio_input_stream;
pub mod audio_output_stream;
pub mod isolated_audio_manager;
pub mod stream_manager;

// Re-export main types for backward compatibility
pub use audio_input_stream::AudioInputStream;
pub use audio_output_stream::AudioOutputStream;
pub use isolated_audio_manager::{AudioCommand, IsolatedAudioManager};
pub use stream_manager::{AudioMetrics, StreamInfo, StreamManager};

// Additional internal type re-exports that may be needed
// These ensure types are available within the module hierarchy
