// Audio recording module - Audio recording and capture functionality
//
// This module provides comprehensive audio recording capabilities with
// a modular architecture for maintainability and clear separation of concerns.

pub mod types;
pub mod encoders;
pub mod filename_generation;
pub mod silence_detection;
pub mod recording_writer;
pub mod recording_service;

// Re-export main public API - types
pub use types::{
    RecordingConfig, RecordingStatus, RecordingHistoryEntry, RecordingSession,
    RecordingFormat, RecordingMetadata, RecordingCommand, RecordingPresets,
    Mp3Settings, FlacSettings, WavSettings,
};

// Re-export main public API - services
pub use recording_service::{RecordingService, RecordingStatistics};

// Re-export encoder interface for advanced usage
pub use encoders::{AudioEncoder, EncoderFactory, EncoderMetadata};

// Re-export filename utilities
pub use filename_generation::{FilenameGenerator, FilenameTemplates, PathManager, TemplateVariables};

// Re-export audio analysis tools
pub use silence_detection::{
    SilenceDetector, AudioQualityAnalyzer, SilenceAnalysis, 
    AudioQuality, SilenceDetectorStats
};

// Re-export recording writer for advanced usage
pub use recording_writer::{RecordingWriter, RecordingWriterManager};