// Audio mixer module - Virtual mixer with audio transformation

// Core modules for mixer functionality
pub mod pipeline;
pub mod resampling;
pub mod stream_management;
pub mod queue_manager;

// Re-export stream management types
pub use stream_management::StreamInfo;

// Re-export pipeline types
pub use pipeline::AudioPipeline;
