// File I/O and recording management for audio recording
//
// This module handles the core file writing operations, session management,
// and coordination between encoders and storage. It provides thread-safe
// recording operations with proper error handling and resource cleanup.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};
use tracing::{info, warn, debug};

use super::types::{RecordingConfig, RecordingSession, RecordingStatus, RecordingHistoryEntry};
use super::encoders::{AudioEncoder, EncoderFactory};
use super::filename_generation::{FilenameGenerator, PathManager};
use super::silence_detection::{SilenceDetector, AudioQualityAnalyzer};

/// Individual recording writer handling one recording session
pub struct RecordingWriter {
    session: RecordingSession,
    file_writer: Option<BufWriter<File>>,
    encoder: Box<dyn AudioEncoder>,
    silence_detector: Option<SilenceDetector>,
    quality_analyzer: AudioQualityAnalyzer,
    is_writing: bool,
    bytes_written: u64,
    samples_processed: u64,
}

impl RecordingWriter {
    /// Create a new recording writer
    pub async fn new(config: RecordingConfig) -> Result<Self> {
        // Generate filename and ensure directory exists
        let filename_generator = FilenameGenerator::new();
        let filename = filename_generator.generate(&config)?;
        let file_path = config.output_directory.join(&filename);
        
        // Ensure output directory exists and is safe
        PathManager::ensure_directory_exists(&config.output_directory).await?;
        if !PathManager::is_safe_recording_path(&config.output_directory) {
            return Err(anyhow::anyhow!("Unsafe recording path: {}", config.output_directory.display()));
        }
        
        // Make filename unique if it already exists
        let unique_file_path = PathManager::make_unique_filename(&file_path);
        
        // Create encoder
        let mut encoder = EncoderFactory::create_encoder(&config)?;
        encoder.initialize(&config)?;
        
        // Create silence detector if enabled
        let silence_detector = SilenceDetector::from_config(&config);
        
        // Create quality analyzer
        let quality_analyzer = AudioQualityAnalyzer::new(config.sample_rate);
        
        // Create session
        let session = RecordingSession::new(config, unique_file_path);
        
        info!("Created recording writer for: {}", session.current_file_path.display());
        
        Ok(Self {
            session,
            file_writer: None,
            encoder,
            silence_detector,
            quality_analyzer,
            is_writing: false,
            bytes_written: 0,
            samples_processed: 0,
        })
    }
    
    /// Start recording - opens file and initializes encoder
    pub async fn start(&mut self) -> Result<()> {
        if self.is_writing {
            return Err(anyhow::anyhow!("Recording already started"));
        }
        
        // Create file and buffered writer
        let file = File::create(&self.session.current_file_path).await
            .with_context(|| format!("Failed to create recording file: {}", self.session.current_file_path.display()))?;
        
        self.file_writer = Some(BufWriter::new(file));
        self.is_writing = true;
        self.session.start_time = SystemTime::now();
        
        info!("Started recording to: {}", self.session.current_file_path.display());
        Ok(())
    }
    
    /// Process audio samples and write to file
    pub async fn process_samples(&mut self, samples: &[f32]) -> Result<bool> {
        if !self.is_writing || self.session.is_paused {
            return Ok(true); // Continue recording
        }
        
        if samples.is_empty() {
            return Ok(true);
        }
        
        // Update session statistics
        self.samples_processed += samples.len() as u64;
        let sample_rate = self.session.config.sample_rate as f64;
        self.session.duration_seconds = self.samples_processed as f64 / sample_rate / self.session.config.channels as f64;
        
        // Analyze audio quality
        let quality = self.quality_analyzer.analyze_samples(samples);
        if !quality.is_acceptable() {
            warn!("Poor audio quality detected: {} ({}% clipping)", 
                  quality.get_quality_text(), quality.clip_rate_percent);
        }
        
        // Check for silence and auto-stop
        let mut should_stop = false;
        if let Some(detector) = &mut self.silence_detector {
            let analysis = detector.process_samples(samples);
            if analysis.should_auto_stop {
                info!("Auto-stopping recording due to silence: {:.1}s", analysis.silence_duration_seconds());
                should_stop = true;
            }
        }
        
        // Check duration and size limits
        if self.session.should_auto_stop_duration() {
            info!("Auto-stopping recording due to duration limit: {:.1}min", 
                  self.session.duration_seconds / 60.0);
            should_stop = true;
        }
        
        if self.session.should_auto_stop_size() {
            info!("Auto-stopping recording due to file size limit: {}MB", 
                  self.session.file_size_bytes / (1024 * 1024));
            should_stop = true;
        }
        
        // Encode audio data
        let encoded_data = self.encoder.encode(samples)
            .with_context(|| "Failed to encode audio samples")?;
        
        // Write encoded data to file
        if !encoded_data.is_empty() {
            if let Some(writer) = &mut self.file_writer {
                writer.write_all(&encoded_data).await
                    .with_context(|| "Failed to write encoded data to file")?;
                
                self.bytes_written += encoded_data.len() as u64;
                self.session.file_size_bytes = self.bytes_written;
            }
        }
        
        // Return whether to continue recording
        Ok(!should_stop)
    }
    
    /// Pause recording
    pub fn pause(&mut self) -> Result<()> {
        self.session.is_paused = true;
        info!("Recording paused");
        Ok(())
    }
    
    /// Resume recording
    pub fn resume(&mut self) -> Result<()> {
        self.session.is_paused = false;
        info!("Recording resumed");
        Ok(())
    }
    
    /// Stop recording and finalize file
    pub async fn stop(&mut self) -> Result<RecordingHistoryEntry> {
        if !self.is_writing {
            return Err(anyhow::anyhow!("Recording not started"));
        }
        
        // Finalize encoder and write any remaining data
        let final_data = self.encoder.finalize()
            .with_context(|| "Failed to finalize encoder")?;
        
        if !final_data.is_empty() {
            if let Some(writer) = &mut self.file_writer {
                writer.write_all(&final_data).await
                    .with_context(|| "Failed to write final encoded data")?;
                writer.flush().await
                    .with_context(|| "Failed to flush file writer")?;
                
                self.bytes_written += final_data.len() as u64;
                self.session.file_size_bytes = self.bytes_written;
            }
        }
        
        // Close file writer
        if let Some(mut writer) = self.file_writer.take() {
            writer.flush().await
                .with_context(|| "Failed to flush writer on close")?;
        }
        
        self.is_writing = false;
        let end_time = SystemTime::now();
        
        // Create history entry
        let history_entry = RecordingHistoryEntry::from_session(&self.session, end_time);
        
        info!("Recording completed: {} ({} samples, {:.1}s, {} bytes)", 
              self.session.current_file_path.display(),
              self.samples_processed,
              self.session.duration_seconds,
              self.bytes_written);
        
        Ok(history_entry)
    }
    
    /// Get current recording status
    pub fn get_status(&self) -> RecordingStatus {
        // Calculate available disk space for recording directory
        let available_space_gb = {
            let recording_dir = self.session.current_file_path.parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            
            match super::filename_generation::PathManager::get_available_space(recording_dir) {
                Ok(bytes) => (bytes as f64) / (1024.0 * 1024.0 * 1024.0), // Convert to GB
                Err(_) => 100.0, // Fallback to 100GB if check fails
            }
        };
        
        RecordingStatus {
            is_recording: self.is_writing,
            is_paused: self.session.is_paused,
            session: if self.is_writing { Some(self.session.clone()) } else { None },
            active_writers_count: if self.is_writing { 1 } else { 0 },
            available_space_gb,             // **RESTORED**: Real disk space checking
            total_recordings: 1,            // **RESTORED**: This writer represents 1 recording
            active_recordings: if self.is_writing { vec![self.session.id.clone()] } else { vec![] }, // **RESTORED**: Frontend expects this
        }
    }
    
    /// Get recording session info
    pub fn get_session(&self) -> &RecordingSession {
        &self.session
    }
    
    /// Update recording metadata
    pub fn update_metadata(&mut self, metadata: super::types::RecordingMetadata) {
        self.session.config.metadata = metadata;
        debug!("Updated recording metadata");
    }
    
    /// Get encoder metadata
    pub fn get_encoder_metadata(&self) -> super::encoders::EncoderMetadata {
        self.encoder.get_metadata()
    }
}

impl Drop for RecordingWriter {
    fn drop(&mut self) {
        if self.is_writing {
            warn!("RecordingWriter dropped while still recording - file may be incomplete");
        }
    }
}

/// Manager for multiple concurrent recording writers
pub struct RecordingWriterManager {
    writers: Arc<tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<RecordingWriter>>>>>,
    history: Arc<tokio::sync::Mutex<Vec<RecordingHistoryEntry>>>,
}

impl RecordingWriterManager {
    /// Create a new recording writer manager
    pub fn new() -> Self {
        Self {
            writers: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            history: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }
    
    /// Start a new recording
    pub async fn start_recording(&self, config: RecordingConfig) -> Result<String> {
        let writer = RecordingWriter::new(config).await?;
        let session_id = writer.get_session().id.clone();
        
        let writer = Arc::new(tokio::sync::Mutex::new(writer));
        
        // Start the recording
        {
            let mut w = writer.lock().await;
            w.start().await?;
        }
        
        // Add to active writers
        {
            let mut writers = self.writers.lock().await;
            writers.insert(session_id.clone(), writer);
        }
        
        Ok(session_id)
    }
    
    /// Stop a recording
    pub async fn stop_recording(&self, session_id: &str) -> Result<RecordingHistoryEntry> {
        let writer = {
            let mut writers = self.writers.lock().await;
            writers.remove(session_id)
                .ok_or_else(|| anyhow::anyhow!("Recording session not found: {}", session_id))?
        };
        
        let history_entry = {
            let mut w = writer.lock().await;
            w.stop().await?
        };
        
        // Add to history
        {
            let mut history = self.history.lock().await;
            history.push(history_entry.clone());
        }
        
        Ok(history_entry)
    }
    
    /// Process audio samples for a recording
    pub async fn process_samples(&self, session_id: &str, samples: &[f32]) -> Result<bool> {
        let writer = {
            let writers = self.writers.lock().await;
            writers.get(session_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Recording session not found: {}", session_id))?
        };
        
        let mut w = writer.lock().await;
        w.process_samples(samples).await
    }
    
    /// Get overall recording status
    pub fn get_status(&self) -> Result<RecordingStatus> {
        // Use try_lock for non-blocking access
        match self.writers.try_lock() {
            Ok(writers) => {
                let active_count = writers.len();
                
                // Get disk space from first writer or use default recording directory
                let available_space_gb = if let Some((_, writer)) = writers.iter().next() {
                    // Get from active writer
                    if let Ok(writer_guard) = writer.try_lock() {
                        let recording_dir = writer_guard.session.current_file_path.parent()
                            .unwrap_or_else(|| std::path::Path::new("."));
                        
                        match super::filename_generation::PathManager::get_available_space(recording_dir) {
                            Ok(bytes) => (bytes as f64) / (1024.0 * 1024.0 * 1024.0),
                            Err(_) => 100.0, // Fallback
                        }
                    } else {
                        100.0 // Fallback if writer locked
                    }
                } else {
                    // No active writers - check default recording directory
                    let default_dir = dirs::audio_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
                    match super::filename_generation::PathManager::get_available_space(&default_dir) {
                        Ok(bytes) => (bytes as f64) / (1024.0 * 1024.0 * 1024.0),
                        Err(_) => 100.0, // Fallback
                    }
                };
                
                // Collect active recording IDs
                let active_recordings: Vec<String> = writers.keys().cloned().collect();
                
                Ok(RecordingStatus {
                    is_recording: active_count > 0,
                    is_paused: false, // Simplified for now
                    session: None, // Would need more complex logic to get session safely
                    active_writers_count: active_count,
                    available_space_gb,                    // **RESTORED**: Real disk space
                    total_recordings: self.history.try_lock().map(|h| h.len()).unwrap_or(0), // **RESTORED**: Count from history
                    active_recordings,                     // **RESTORED**: Frontend expects this
                })
            }
            Err(_) => Ok(RecordingStatus::default())
        }
    }

    /// Get overall recording status (async version to avoid blocking)
    pub async fn get_status_async(&self) -> Result<RecordingStatus> {
        let writers = self.writers.lock().await;
        let active_count = writers.len();
        
        // Get any active session for status and disk space
        let (session, available_space_gb) = if let Some((_, writer)) = writers.iter().next() {
            let w = writer.lock().await;
            let session = w.get_session().clone();
            
            // Calculate disk space for recording directory
            let recording_dir = session.current_file_path.parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            
            let space_gb = match super::filename_generation::PathManager::get_available_space(recording_dir) {
                Ok(bytes) => (bytes as f64) / (1024.0 * 1024.0 * 1024.0),
                Err(_) => 100.0, // Fallback
            };
            
            (Some(session), space_gb)
        } else {
            // No active writers - check default recording directory for disk space
            let default_dir = dirs::audio_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            let space_gb = match super::filename_generation::PathManager::get_available_space(&default_dir) {
                Ok(bytes) => (bytes as f64) / (1024.0 * 1024.0 * 1024.0),
                Err(_) => 100.0, // Fallback
            };
            
            (None, space_gb)
        };
        
        // Collect active recording IDs
        let active_recordings: Vec<String> = writers.keys().cloned().collect();
        
        Ok(RecordingStatus {
            is_recording: active_count > 0,
            is_paused: session.as_ref().map(|s| s.is_paused).unwrap_or(false),
            session,
            active_writers_count: active_count,
            available_space_gb,                    // **RESTORED**: Real disk space
            total_recordings: self.history.lock().await.len(), // **RESTORED**: Count from history
            active_recordings,                     // **RESTORED**: Frontend expects this
        })
    }
    
    /// Get recording history
    pub fn get_history(&self) -> Result<Vec<RecordingHistoryEntry>> {
        match self.history.try_lock() {
            Ok(history) => Ok(history.clone()),
            Err(_) => Ok(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_recording_writer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = RecordingConfig {
            output_directory: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let writer = RecordingWriter::new(config).await;
        assert!(writer.is_ok());
    }
    
    #[tokio::test]
    async fn test_recording_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let config = RecordingConfig {
            output_directory: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let mut writer = RecordingWriter::new(config).await.unwrap();
        
        // Start recording
        assert!(writer.start().await.is_ok());
        assert!(writer.is_writing);
        
        // Process some samples
        let samples = vec![0.1, 0.2, -0.1, -0.2];
        let should_continue = writer.process_samples(&samples).await.unwrap();
        assert!(should_continue);
        
        // Stop recording
        let history = writer.stop().await.unwrap();
        assert!(!writer.is_writing);
        assert!(history.file_size_bytes > 0);
    }
}