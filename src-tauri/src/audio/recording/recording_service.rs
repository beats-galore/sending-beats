// Main recording service implementation and public API
//
// This module provides the high-level recording service interface that
// coordinates all recording functionality. It serves as the main entry
// point for recording operations and manages the interaction between
// different recording subsystems.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};
use colored::Colorize;

use super::encoders::EncoderFactory;
use super::filename_generation::{FilenameGenerator, FilenameTemplates};
use super::recording_writer::RecordingWriterManager;
use super::types::{
    RecordingCommand, RecordingConfig, RecordingHistoryEntry, RecordingMetadata, RecordingPresets,
    RecordingStatus,
};

/// Main recording service that coordinates all recording functionality
pub struct RecordingService {
    writer_manager: Arc<RecordingWriterManager>,
    configs: Arc<Mutex<HashMap<String, RecordingConfig>>>,
    active_session_id: Arc<Mutex<Option<String>>>,
    command_sender: Option<mpsc::UnboundedSender<RecordingCommand>>,
}

impl RecordingService {
    /// Create a new recording service
    pub fn new() -> Self {
        let writer_manager = Arc::new(RecordingWriterManager::new());

        Self {
            writer_manager,
            configs: Arc::new(Mutex::new(HashMap::new())),
            active_session_id: Arc::new(Mutex::new(None)),
            command_sender: None,
        }
    }

    /// Initialize the recording service with command processing and crash recovery
    pub async fn initialize(&mut self) -> Result<Vec<String>> {
        let (tx, mut rx) = mpsc::unbounded_channel::<RecordingCommand>();
        self.command_sender = Some(tx);

        // Initialize crash recovery
        let recovered_files = self.writer_manager.initialize().await?;

        let writer_manager = Arc::clone(&self.writer_manager);
        let active_session_id = Arc::clone(&self.active_session_id);

        // Spawn command processing task
        tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                match command {
                    RecordingCommand::Start(config) => {
                        match writer_manager.start_recording(config).await {
                            Ok(session_id) => {
                                let mut active_id = active_session_id.lock().await;
                                *active_id = Some(session_id.clone());
                                info!("Started recording session: {}", session_id);
                            }
                            Err(e) => {
                                error!("Failed to start recording: {}", e);
                            }
                        }
                    }
                    RecordingCommand::Stop => {
                        let session_id = {
                            let mut active_id = active_session_id.lock().await;
                            active_id.take()
                        };

                        if let Some(session_id) = session_id {
                            match writer_manager.stop_recording(&session_id).await {
                                Ok(history_entry) => {
                                    info!(
                                        "Stopped recording session: {} - {}",
                                        session_id,
                                        history_entry.get_duration_display()
                                    );
                                }
                                Err(e) => {
                                    error!("Failed to stop recording: {}", e);
                                }
                            }
                        }
                    }
                    RecordingCommand::Pause => {
                        // Implementation would need to be added to writer manager
                        info!("Pause command received (not yet implemented)");
                    }
                    RecordingCommand::Resume => {
                        // Implementation would need to be added to writer manager
                        info!("Resume command received (not yet implemented)");
                    }
                    RecordingCommand::UpdateMetadata(metadata) => {
                        info!(
                            "Metadata update received with {} fields",
                            metadata.get_display_fields().len()
                        );

                        // Update metadata for active recording session
                        let active_id = active_session_id.lock().await;
                        if let Some(ref session_id) = *active_id {
                            match writer_manager
                                .update_session_metadata(session_id, metadata)
                                .await
                            {
                                Ok(()) => info!("Session metadata updated successfully"),
                                Err(e) => error!("Failed to update session metadata: {}", e),
                            }
                        } else {
                            warn!("No active recording session to update metadata");
                        }
                    }
                }
            }
        });

        info!(
            "Recording service initialized, recovered {} temp files",
            recovered_files.len()
        );
        Ok(recovered_files)
    }

    /// Start a new recording with the given configuration and RTRB consumer
    pub async fn start_recording(
        &self,
        config: RecordingConfig,
        rtrb_consumer: rtrb::Consumer<f32>,
    ) -> Result<String> {
        // Validate configuration
        config.validate()?;

        // Store configuration
        {
            let mut configs = self.configs.lock().await;
            configs.insert(config.id.clone(), config.clone());
        }

        // Start recording directly and get session ID
        let session_id = self.writer_manager.start_recording(config).await?;

        // Also update active session tracking
        {
            let mut active_id = self.active_session_id.lock().await;
            *active_id = Some(session_id.clone());
        }

        // Spawn audio processing task to continuously read samples from RTRB and write to recording
        let writer_manager = Arc::clone(&self.writer_manager);
        let processing_session_id = session_id.clone();
        let mut consumer = rtrb_consumer;
        tokio::spawn(async move {
            info!(
                "🎵 {}: Starting RTRB consumer loop for session: {}",
                "RECORDING_RTRB_CONSUMER".red(),
                processing_session_id
            );
            let mut sample_count = 0u64;
            let mut batch_count = 0u64;
            let mut sample_buffer = Vec::with_capacity(4096); // Buffer for collecting samples

            loop {
                sample_buffer.clear();

                // **RTRB DRAIN STRATEGY**: Collect available samples into buffer
                loop {
                    match consumer.pop() {
                        Ok(sample) => {
                            sample_buffer.push(sample);
                            // Limit buffer size to prevent excessive memory usage
                            if sample_buffer.len() >= 4096 {
                                break;
                            }
                        }
                        Err(_) => break, // No more samples available
                    }
                }

                // Process collected samples if any
                if !sample_buffer.is_empty() {
                    batch_count += 1;
                    sample_count += sample_buffer.len() as u64;

                    // Log first few batches to see if we're getting audio, then every 100 batches
                    if batch_count <= 5 || batch_count % 100 == 0 {
                        info!(
                            "🎵 {}: Processing batch #{}, samples received: {}, total samples: {}",
                            "RECORDING_RTRB_BATCH".red(),
                            batch_count,
                            sample_buffer.len(),
                            sample_count
                        );
                    }

                    // Process the audio samples for this recording session
                    match writer_manager
                        .process_samples(&processing_session_id, &sample_buffer)
                        .await
                    {
                        Ok(should_continue) => {
                            if !should_continue {
                                info!(
                                    "🛑 {}: Auto-stop triggered for session: {}",
                                    "RECORDING_AUTO_STOP".red(),
                                    processing_session_id
                                );
                                break;
                            }
                        }
                        Err(e) => {
                            error!(
                                "❌ {}: Failed to process audio samples for session {}: {}",
                                "RECORDING_WRITE_ERROR".red(),
                                processing_session_id, e
                            );
                            error!(
                                "❌ Error occurred at batch #{}, total samples processed: {}",
                                batch_count, sample_count
                            );
                            break;
                        }
                    }
                } else {
                    // No samples available, yield CPU briefly
                    tokio::time::sleep(std::time::Duration::from_micros(100)).await;
                }
            }

            info!("🔚 {}: RTRB consumer loop ended for session: {} (processed {} batches, {} total samples)",
                  "RECORDING_RTRB_CONSUMER".red(),
                  processing_session_id, batch_count, sample_count);
        });

        info!("Recording service started session: {}", session_id);
        Ok(session_id)
    }

    /// Stop the current recording
    pub async fn stop_recording(&self) -> Result<Option<RecordingHistoryEntry>> {
        let session_id = {
            let mut active_id = self.active_session_id.lock().await;
            active_id.take()
        };

        if let Some(session_id) = session_id {
            match self.writer_manager.stop_recording(&session_id).await {
                Ok(history_entry) => {
                    info!("Recording service stopped session: {}", session_id);
                    Ok(Some(history_entry))
                }
                Err(e) => {
                    error!("Failed to stop recording session {}: {}", session_id, e);
                    Err(e)
                }
            }
        } else {
            Ok(None) // No active recording
        }
    }

    /// Process audio samples for recording
    pub async fn process_audio_samples(&self, samples: &[f32]) -> Result<()> {
        let session_id = {
            let active_id = self.active_session_id.lock().await;
            active_id.clone()
        };

        if let Some(session_id) = session_id {
            let should_continue = self
                .writer_manager
                .process_samples(&session_id, samples)
                .await?;

            if !should_continue {
                // Auto-stop triggered
                self.stop_recording().await?;
            }
        }

        Ok(())
    }

    /// Get current recording status
    pub async fn get_status(&self) -> RecordingStatus {
        // Use async version to get complete status with session info
        match self.writer_manager.get_status_async().await {
            Ok(status) => status,
            Err(_) => RecordingStatus::default(),
        }
    }

    /// Get recording history
    pub async fn get_history(&self) -> Vec<RecordingHistoryEntry> {
        self.writer_manager.get_history().unwrap_or_default()
    }

    /// Update metadata for the current recording session
    pub async fn update_session_metadata(&self, metadata: RecordingMetadata) -> Result<()> {
        if let Some(sender) = &self.command_sender {
            sender.send(RecordingCommand::UpdateMetadata(metadata))?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Recording service not initialized"))
        }
    }

    /// Save a recording configuration
    pub async fn save_config(&self, config: RecordingConfig) -> Result<()> {
        config.validate()?;

        let mut configs = self.configs.lock().await;
        configs.insert(config.id.clone(), config.clone());

        info!("Saved recording configuration: {}", config.name);
        Ok(())
    }

    /// Load a recording configuration
    pub async fn load_config(&self, config_id: &str) -> Result<RecordingConfig> {
        let configs = self.configs.lock().await;

        configs
            .get(config_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Configuration not found: {}", config_id))
    }

    /// Get all saved configurations
    pub async fn get_configs(&self) -> Vec<RecordingConfig> {
        match self.configs.try_lock() {
            Ok(configs) => configs.values().cloned().collect(),
            Err(_) => {
                eprintln!("Failed to lock configs");
                Vec::new()
            }
        }
    }

    /// Delete a configuration
    pub async fn delete_config(&self, config_id: &str) -> Result<()> {
        let mut configs = self.configs.lock().await;

        configs
            .remove(config_id)
            .ok_or_else(|| anyhow::anyhow!("Configuration not found: {}", config_id))?;

        info!("Deleted recording configuration: {}", config_id);
        Ok(())
    }

    /// Get available filename templates
    pub fn get_filename_templates(&self) -> Vec<(&'static str, &'static str)> {
        FilenameTemplates::all_templates()
    }

    /// Validate a filename template
    pub fn validate_filename_template(&self, template: &str) -> Result<Vec<String>> {
        let generator = FilenameGenerator::new();
        generator.validate_template(template)
    }

    /// Get supported recording formats
    pub fn get_supported_formats(&self) -> Vec<&'static str> {
        EncoderFactory::supported_formats()
    }

    /// Check if a format is supported
    pub fn is_format_supported(&self, extension: &str) -> bool {
        EncoderFactory::is_format_supported(extension)
    }

    /// Get preset configurations
    pub fn get_presets(&self) -> Vec<(&'static str, RecordingConfig)> {
        vec![
            (
                "High Quality Stereo",
                RecordingPresets::high_quality_stereo(),
            ),
            ("MP3 Standard", RecordingPresets::mp3_standard()),
            ("FLAC Lossless", RecordingPresets::flac_lossless()),
            ("Podcast", RecordingPresets::podcast()),
        ]
    }

    /// Create a configuration from a preset
    pub fn create_from_preset(
        &self,
        preset_name: &str,
        custom_name: Option<String>,
    ) -> Result<RecordingConfig> {
        let mut config = match preset_name {
            "High Quality Stereo" => RecordingPresets::high_quality_stereo(),
            "MP3 Standard" => RecordingPresets::mp3_standard(),
            "FLAC Lossless" => RecordingPresets::flac_lossless(),
            "Podcast" => RecordingPresets::podcast(),
            _ => return Err(anyhow::anyhow!("Unknown preset: {}", preset_name)),
        };

        if let Some(name) = custom_name {
            config.name = name;
        }

        // Generate new ID
        config.id = uuid::Uuid::new_v4().to_string();

        Ok(config)
    }

    /// Get recording statistics
    pub async fn get_statistics(&self) -> Result<RecordingStatistics> {
        let history = self.get_history().await;
        let status = self.get_status().await;

        let total_recordings = history.len();
        let total_duration: f64 = history.iter().map(|h| h.duration_seconds).sum();
        let total_size: u64 = history.iter().map(|h| h.file_size_bytes).sum();

        let average_duration = if total_recordings > 0 {
            total_duration / total_recordings as f64
        } else {
            0.0
        };

        let formats_used: HashMap<String, usize> =
            history.iter().fold(HashMap::new(), |mut acc, h| {
                let format = h.format.get_format_name();
                *acc.entry(format.to_string()).or_insert(0) += 1;
                acc
            });

        Ok(RecordingStatistics {
            total_recordings,
            total_duration_seconds: total_duration,
            total_size_bytes: total_size,
            average_duration_seconds: average_duration,
            is_currently_recording: status.is_recording,
            active_sessions: status.active_writers_count,
            formats_used,
        })
    }
}

impl Default for RecordingService {
    fn default() -> Self {
        Self::new()
    }
}

/// Recording service statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingStatistics {
    pub total_recordings: usize,
    pub total_duration_seconds: f64,
    pub total_size_bytes: u64,
    pub average_duration_seconds: f64,
    pub is_currently_recording: bool,
    pub active_sessions: usize,
    pub formats_used: HashMap<String, usize>,
}

impl RecordingStatistics {
    /// Get total duration as human-readable string
    pub fn get_total_duration_display(&self) -> String {
        let total_seconds = self.total_duration_seconds as u64;
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        if hours > 0 {
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            format!("{:02}:{:02}", minutes, seconds)
        }
    }

    /// Get total size as human-readable string
    pub fn get_total_size_display(&self) -> String {
        let bytes = self.total_size_bytes as f64;
        if bytes < 1024.0 {
            format!("{:.0} B", bytes)
        } else if bytes < 1024.0 * 1024.0 {
            format!("{:.1} KB", bytes / 1024.0)
        } else if bytes < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.1} MB", bytes / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", bytes / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_recording_service_creation() {
        let service = RecordingService::new();
        assert!(service.get_status().is_ok());
    }

    #[test]
    fn test_preset_creation() {
        let service = RecordingService::new();
        let presets = service.get_presets();

        assert!(!presets.is_empty());
        assert!(presets
            .iter()
            .any(|(name, _)| *name == "High Quality Stereo"));
    }

    #[test]
    fn test_format_support() {
        let service = RecordingService::new();
        let formats = service.get_supported_formats();

        assert!(formats.contains(&"wav"));
        assert!(service.is_format_supported("wav"));
        assert!(service.is_format_supported("mp3"));
    }

    #[test]
    fn test_filename_templates() {
        let service = RecordingService::new();
        let templates = service.get_filename_templates();

        assert!(!templates.is_empty());
        assert!(templates
            .iter()
            .any(|(_, template)| template.contains("{timestamp}")));
    }
}
