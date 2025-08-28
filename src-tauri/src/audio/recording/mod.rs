// Audio recording module - Audio recording and capture functionality  
//
// This module provides comprehensive audio recording capabilities:
// - service: Recording service management and coordination

pub mod service;

// Re-export service types
pub use service::{
    RecordingService, RecordingConfig, RecordingStatus, RecordingHistoryEntry,
    RecordingFormat, RecordingMetadata, RecordingSession, RecordingWriter, RecordingCommand,
};