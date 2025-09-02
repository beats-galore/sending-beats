// File I/O and recording management for audio recording
//
// This module handles the core file writing operations, session management,
// and coordination between encoders and storage. It provides thread-safe
// recording operations with proper error handling and resource cleanup.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};
use tracing::{info, warn, debug};

use super::types::{RecordingConfig, RecordingSession, RecordingStatus, RecordingHistoryEntry, RecordingMetadata, RecordingFormat};
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
    
    /// Start recording - opens file and initializes encoder with temporary file support
    pub async fn start(&mut self) -> Result<()> {
        if self.is_writing {
            return Err(anyhow::anyhow!("Recording already started"));
        }
        
        // Get write path and update start time
        let write_path = self.session.get_write_path().clone();
        self.session.start_time = SystemTime::now();
        
        // Create file and buffered writer
        let file = File::create(&write_path).await
            .with_context(|| format!("Failed to create recording file: {}", write_path.display()))?;
        
        self.file_writer = Some(BufWriter::new(file));
        self.is_writing = true;
        
        // Update session metadata with encoder info
        let encoder_name = self.encoder.get_metadata().encoder_name.unwrap_or("WAV".to_string());
        self.session.metadata.set_technical_metadata(&self.session.config, &encoder_name);
        
        info!("Started recording to temporary file: {}", write_path.display());
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
        let old_duration = self.session.duration_seconds;
        self.session.duration_seconds = self.samples_processed as f64 / sample_rate / self.session.config.channels as f64;
        
        // Debug log session updates every 1000 sample batches to track progress
        if self.samples_processed % (sample_rate as u64 * self.session.config.channels as u64) == 0 {
            info!("📊 Session {} stats: duration {:.1}s (was {:.1}s), samples {}, file_size {}B", 
                  self.session.id, self.session.duration_seconds, old_duration, self.samples_processed, self.session.file_size_bytes);
        }
        
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
    
    /// Stop recording and finalize file with temporary file handling
    pub async fn stop(&mut self) -> Result<RecordingHistoryEntry> {
        if !self.is_writing {
            return Err(anyhow::anyhow!("Recording not started"));
        }
        
        // Update final metadata
        self.session.metadata.set_duration(self.session.duration_seconds);
        
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
        
        // Close file writer before moving temp file
        if let Some(mut writer) = self.file_writer.take() {
            writer.flush().await
                .with_context(|| "Failed to flush writer on close")?;
        }
        
        self.is_writing = false;
        let end_time = SystemTime::now();
        
        // Move temporary file to final destination (atomic operation)
        self.session.finalize_recording()
            .with_context(|| "Failed to finalize recording file")?;
        
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
            warn!("RecordingWriter dropped while still recording - cleaning up temporary file");
            if let Err(e) = self.session.cleanup_temp_file() {
                warn!("Failed to cleanup temporary file on drop: {}", e);
            }
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
    
    /// Initialize and scan for recoverable temporary files
    pub async fn initialize(&self) -> Result<Vec<String>> {
        let mut recovered_files = Vec::new();
        
        // Get common recording directories to scan
        let mut scan_dirs = Vec::new();
        
        // Add default audio directory
        if let Some(audio_dir) = dirs::audio_dir() {
            scan_dirs.push(audio_dir);
        }
        
        // Add current directory as fallback
        scan_dirs.push(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        
        for dir in scan_dirs {
            if let Ok(recovered) = self.scan_directory_for_temp_files(&dir).await {
                recovered_files.extend(recovered);
            }
        }
        
        info!("Recording writer manager initialized, recovered {} temp files", recovered_files.len());
        Ok(recovered_files)
    }
    
    /// Scan a directory for recoverable temporary files
    async fn scan_directory_for_temp_files(&self, dir: &PathBuf) -> Result<Vec<String>> {
        use tokio::fs;
        
        let mut recovered = Vec::new();
        
        if !dir.exists() {
            return Ok(recovered);
        }
        
        let mut entries = fs::read_dir(dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            // Look for .tmp files
            if let Some(extension) = path.extension() {
                if extension == "tmp" {
                    // Check if it's one of our audio temp files
                    if let Some(stem) = path.file_stem() {
                        let stem_str = stem.to_string_lossy();
                        if stem_str.ends_with(".wav") || stem_str.ends_with(".mp3") || stem_str.ends_with(".flac") {
                            // Try to recover this file
                            match self.attempt_recovery(&path).await {
                                Ok(final_path) => {
                                    info!("Recovered temporary file: {} -> {}", path.display(), final_path);
                                    recovered.push(final_path);
                                }
                                Err(e) => {
                                    warn!("Failed to recover temp file {}: {}", path.display(), e);
                                    // Clean up failed recovery attempt
                                    let _ = fs::remove_file(&path).await;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(recovered)
    }
    
    /// Attempt to recover a temporary file
    async fn attempt_recovery(&self, temp_path: &PathBuf) -> Result<String> {
        use tokio::fs;
        
        // Get the final path by removing .tmp extension
        let final_path = if let Some(stem) = temp_path.file_stem() {
            temp_path.with_file_name(stem)
        } else {
            return Err(anyhow::anyhow!("Invalid temp file name"));
        };
        
        // Make sure the final path doesn't already exist
        let unique_final_path = super::filename_generation::PathManager::make_unique_filename(&final_path);
        
        // Move the temp file to the final location
        fs::rename(temp_path, &unique_final_path).await?;
        
        // Add to history as a recovered recording
        let metadata = RecordingMetadata::default();
        let file_size = fs::metadata(&unique_final_path).await?.len();
        
        let history_entry = RecordingHistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            config_name: "Recovered Recording".to_string(),
            file_path: unique_final_path.clone(),
            start_time: SystemTime::now(), // Unknown original start time
            end_time: SystemTime::now(),
            duration_seconds: 0.0, // Unknown duration
            file_size_bytes: file_size,
            format: RecordingFormat::default(), // Assume WAV
            metadata,
        };
        
        {
            let mut history = self.history.lock().await;
            history.push(history_entry);
        }
        
        Ok(unique_final_path.to_string_lossy().to_string())
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


    /// Update metadata for a specific recording session
    pub async fn update_session_metadata(&self, session_id: &str, metadata: RecordingMetadata) -> Result<()> {
        let writers = self.writers.lock().await;
        if let Some(writer_arc) = writers.get(session_id) {
            let mut writer = writer_arc.lock().await;
            writer.session.update_metadata(metadata);
            info!("Updated metadata for session {}", session_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Recording session {} not found", session_id))
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