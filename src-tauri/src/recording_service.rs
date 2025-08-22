use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn, error};
use uuid::Uuid;

/// Recording format options
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RecordingFormat {
    #[serde(rename = "mp3")]
    Mp3 { bitrate: u32 },
    #[serde(rename = "flac")]
    Flac { compression_level: u8 },
    #[serde(rename = "wav")]
    Wav,
}

impl Default for RecordingFormat {
    fn default() -> Self {
        Self::Mp3 { bitrate: 320 }
    }
}

/// Audio metadata for recordings
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RecordingMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub comment: Option<String>,
    pub year: Option<u16>,
}

/// Recording configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingConfig {
    pub id: String,
    pub name: String,
    pub format: RecordingFormat,
    pub output_directory: PathBuf,
    pub filename_template: String, // e.g., "{timestamp}_{title}" 
    pub metadata: RecordingMetadata,
    
    // Advanced options
    pub auto_stop_on_silence: bool,
    pub silence_threshold_db: f32,    // -60.0 dB
    pub silence_duration_sec: f32,    // 5.0 seconds
    pub max_duration_minutes: Option<u32>,
    pub max_file_size_mb: Option<u64>,
    pub split_on_interval_minutes: Option<u32>,
    
    // Quality settings
    pub sample_rate: u32,             // 48000 Hz
    pub channels: u16,                // 2 (stereo)
    pub bit_depth: u16,               // 24-bit
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: "Default Recording".to_string(),
            format: RecordingFormat::default(),
            output_directory: dirs::audio_dir().unwrap_or_else(|| PathBuf::from(".")),
            filename_template: "{timestamp}_{title}".to_string(),
            metadata: RecordingMetadata::default(),
            auto_stop_on_silence: false,
            silence_threshold_db: -60.0,
            silence_duration_sec: 5.0,
            max_duration_minutes: None,
            max_file_size_mb: None,
            split_on_interval_minutes: None,
            sample_rate: 48000,
            channels: 2,
            bit_depth: 24,
        }
    }
}

/// Current recording session information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingSession {
    pub id: String,
    pub config: RecordingConfig,
    pub start_time: SystemTime,
    pub current_file_path: PathBuf,
    pub duration_seconds: f64,
    pub file_size_bytes: u64,
    pub current_levels: (f32, f32), // (left, right) RMS levels
    pub is_paused: bool,
    
    // Statistics
    pub samples_written: u64,
    pub peak_levels: (f32, f32), // Peak levels since recording started
    pub silence_detected_duration: f32,
}

/// Recording service status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingStatus {
    pub is_recording: bool,
    pub current_session: Option<RecordingSession>,
    pub available_space_gb: f64,
    pub total_recordings: usize,
    pub active_recordings: Vec<String>, // Recording IDs
}

/// Recording history entry
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingHistoryEntry {
    pub id: String,
    pub file_path: PathBuf,
    pub config: RecordingConfig,
    pub start_time: SystemTime,
    pub duration_seconds: f64,
    pub file_size_bytes: u64,
    pub created_at: SystemTime,
}

/// Audio encoder trait
pub trait AudioEncoder: Send + Sync {
    fn encode_samples(&mut self, samples: &[f32]) -> Result<Vec<u8>>;
    fn finalize(&mut self) -> Result<Vec<u8>>;
    fn estimated_bitrate(&self) -> u32;
}

/// WAV encoder implementation
pub struct WavEncoder {
    sample_rate: u32,
    channels: u16,
    bit_depth: u16,
    samples_written: u64,
}

impl WavEncoder {
    pub fn new(sample_rate: u32, channels: u16, bit_depth: u16) -> Self {
        Self {
            sample_rate,
            channels,
            bit_depth,
            samples_written: 0,
        }
    }
    
    pub fn write_header(&self) -> Vec<u8> {
        let mut header = Vec::new();
        
        // RIFF chunk
        header.extend_from_slice(b"RIFF");
        header.extend_from_slice(&[0, 0, 0, 0]); // File size - will update later
        header.extend_from_slice(b"WAVE");
        
        // fmt chunk
        header.extend_from_slice(b"fmt ");
        header.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        header.extend_from_slice(&1u16.to_le_bytes());  // Audio format (PCM)
        header.extend_from_slice(&self.channels.to_le_bytes());
        header.extend_from_slice(&self.sample_rate.to_le_bytes());
        
        let byte_rate = self.sample_rate * self.channels as u32 * (self.bit_depth / 8) as u32;
        header.extend_from_slice(&byte_rate.to_le_bytes());
        
        let block_align = self.channels * (self.bit_depth / 8);
        header.extend_from_slice(&block_align.to_le_bytes());
        header.extend_from_slice(&self.bit_depth.to_le_bytes());
        
        // data chunk header
        header.extend_from_slice(b"data");
        header.extend_from_slice(&[0, 0, 0, 0]); // Data size - will update later
        
        header
    }
}

impl AudioEncoder for WavEncoder {
    fn encode_samples(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        
        match self.bit_depth {
            16 => {
                for &sample in samples {
                    let sample_i16 = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                    output.extend_from_slice(&sample_i16.to_le_bytes());
                }
            },
            24 => {
                for &sample in samples {
                    let sample_i32 = (sample.clamp(-1.0, 1.0) * 8388607.0) as i32;
                    let bytes = sample_i32.to_le_bytes();
                    output.extend_from_slice(&bytes[0..3]); // 24-bit = 3 bytes
                }
            },
            32 => {
                for &sample in samples {
                    output.extend_from_slice(&sample.to_le_bytes());
                }
            },
            _ => return Err(anyhow::anyhow!("Unsupported bit depth: {}", self.bit_depth)),
        }
        
        self.samples_written += samples.len() as u64;
        Ok(output)
    }
    
    fn finalize(&mut self) -> Result<Vec<u8>> {
        // Return empty vec - WAV finalization handled by file writer
        Ok(Vec::new())
    }
    
    fn estimated_bitrate(&self) -> u32 {
        self.sample_rate * self.channels as u32 * self.bit_depth as u32 / 1000
    }
}

/// Recording writer that handles file I/O and encoding
pub struct RecordingWriter {
    session: RecordingSession,
    encoder: Box<dyn AudioEncoder>,
    file: File,
    audio_rx: broadcast::Receiver<Vec<f32>>,
    command_rx: mpsc::Receiver<RecordingCommand>,
    silence_detector: SilenceDetector,
}

/// Silence detection utility
pub struct SilenceDetector {
    threshold_db: f32,
    required_duration: Duration,
    current_silence_duration: Duration,
    last_check: SystemTime,
}

impl SilenceDetector {
    pub fn new(threshold_db: f32, required_duration_sec: f32) -> Self {
        Self {
            threshold_db,
            required_duration: Duration::from_secs_f32(required_duration_sec),
            current_silence_duration: Duration::ZERO,
            last_check: SystemTime::now(),
        }
    }
    
    pub fn process_samples(&mut self, samples: &[f32]) -> bool {
        // Calculate RMS level
        let rms = if samples.is_empty() {
            0.0
        } else {
            (samples.iter().map(|&x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
        };
        
        let level_db = if rms > 0.0 {
            20.0 * rms.log10()
        } else {
            -100.0 // Very quiet
        };
        
        let now = SystemTime::now();
        let elapsed = now.duration_since(self.last_check).unwrap_or(Duration::ZERO);
        self.last_check = now;
        
        if level_db < self.threshold_db {
            self.current_silence_duration += elapsed;
        } else {
            self.current_silence_duration = Duration::ZERO;
        }
        
        self.current_silence_duration >= self.required_duration
    }
    
    pub fn reset(&mut self) {
        self.current_silence_duration = Duration::ZERO;
        self.last_check = SystemTime::now();
    }
}

/// Recording commands
#[derive(Debug)]
pub enum RecordingCommand {
    Start(RecordingConfig),
    Stop,
    Pause,
    Resume,
    UpdateMetadata(RecordingMetadata),
}

/// Main recording service
pub struct RecordingService {
    is_running: Arc<tokio::sync::RwLock<bool>>,
    current_session: Arc<Mutex<Option<RecordingSession>>>,
    recording_history: Arc<Mutex<Vec<RecordingHistoryEntry>>>,
    active_writers: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    configs: Arc<RwLock<HashMap<String, RecordingConfig>>>,
    command_tx: mpsc::Sender<RecordingCommand>,
    command_rx: Arc<Mutex<mpsc::Receiver<RecordingCommand>>>,
}

impl RecordingService {
    /// Create a new recording service
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);
        
        Self {
            is_running: Arc::new(RwLock::new(false)),
            current_session: Arc::new(Mutex::new(None)),
            recording_history: Arc::new(Mutex::new(Vec::new())),
            active_writers: Arc::new(Mutex::new(HashMap::new())),
            configs: Arc::new(RwLock::new(HashMap::new())),
            command_tx,
            command_rx: Arc::new(Mutex::new(command_rx)),
        }
    }
    
    /// Start recording with the given configuration
    pub async fn start_recording(
        &self,
        config: RecordingConfig,
        audio_rx: broadcast::Receiver<Vec<f32>>,
    ) -> Result<String> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Err(anyhow::anyhow!("Recording already in progress"));
        }
        
        // Generate filename from template
        let filename = self.generate_filename(&config).await?;
        let file_path = config.output_directory.join(filename);
        
        // Ensure output directory exists
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .context("Failed to create output directory")?;
        }
        
        // Create recording session
        let session = RecordingSession {
            id: Uuid::new_v4().to_string(),
            config: config.clone(),
            start_time: SystemTime::now(),
            current_file_path: file_path.clone(),
            duration_seconds: 0.0,
            file_size_bytes: 0,
            current_levels: (0.0, 0.0),
            is_paused: false,
            samples_written: 0,
            peak_levels: (0.0, 0.0),
            silence_detected_duration: 0.0,
        };
        
        let session_id = session.id.clone();
        
        // Store session
        *self.current_session.lock().await = Some(session.clone());
        *is_running = true;
        
        // Start recording writer task
        self.start_recording_writer(session, audio_rx).await?;
        
        info!("Recording started: {} -> {:?}", session_id, file_path);
        Ok(session_id)
    }
    
    /// Stop current recording
    pub async fn stop_recording(&self) -> Result<Option<RecordingHistoryEntry>> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(None);
        }
        
        // Send stop command
        self.command_tx.send(RecordingCommand::Stop).await?;
        
        // Get current session
        let session = self.current_session.lock().await.take();
        *is_running = false;
        
        if let Some(session) = session {
            // Create history entry
            let history_entry = RecordingHistoryEntry {
                id: session.id.clone(),
                file_path: session.current_file_path.clone(),
                config: session.config.clone(),
                start_time: session.start_time,
                duration_seconds: session.duration_seconds,
                file_size_bytes: session.file_size_bytes,
                created_at: SystemTime::now(),
            };
            
            // Add to history
            self.recording_history.lock().await.push(history_entry.clone());
            
            info!("Recording stopped: {}", session.id);
            Ok(Some(history_entry))
        } else {
            Ok(None)
        }
    }
    
    /// Get current recording status
    pub async fn get_status(&self) -> RecordingStatus {
        let is_recording = *self.is_running.read().await;
        let current_session = self.current_session.lock().await.clone();
        let history = self.recording_history.lock().await;
        
        // Calculate available disk space (simplified)
        let available_space_gb = 100.0; // TODO: Implement actual disk space check
        
        RecordingStatus {
            is_recording,
            current_session,
            available_space_gb,
            total_recordings: history.len(),
            active_recordings: if is_recording { vec!["current".to_string()] } else { vec![] },
        }
    }
    
    /// Generate filename from template
    async fn generate_filename(&self, config: &RecordingConfig) -> Result<String> {
        let mut filename = config.filename_template.clone();
        
        // Replace template variables
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        filename = filename.replace("{timestamp}", &timestamp.to_string());
        filename = filename.replace("{title}", &config.metadata.title.clone().unwrap_or_else(|| "Untitled".to_string()));
        filename = filename.replace("{artist}", &config.metadata.artist.clone().unwrap_or_else(|| "Unknown".to_string()));
        filename = filename.replace("{album}", &config.metadata.album.clone().unwrap_or_else(|| "Unknown".to_string()));
        
        // Add file extension based on format
        let extension = match &config.format {
            RecordingFormat::Mp3 { .. } => "mp3",
            RecordingFormat::Flac { .. } => "flac",
            RecordingFormat::Wav => "wav",
        };
        
        if !filename.ends_with(&format!(".{}", extension)) {
            filename.push('.');
            filename.push_str(extension);
        }
        
        // Sanitize filename
        filename = filename.chars()
            .map(|c| if c.is_alphanumeric() || "._-".contains(c) { c } else { '_' })
            .collect();
        
        Ok(filename)
    }
    
    /// Start the recording writer task
    async fn start_recording_writer(
        &self,
        session: RecordingSession,
        audio_rx: broadcast::Receiver<Vec<f32>>,
    ) -> Result<()> {
        // Create encoder based on format
        let encoder: Box<dyn AudioEncoder> = match &session.config.format {
            RecordingFormat::Wav => Box::new(WavEncoder::new(
                session.config.sample_rate,
                session.config.channels,
                session.config.bit_depth,
            )),
            RecordingFormat::Mp3 { .. } => {
                return Err(anyhow::anyhow!("MP3 encoding not yet implemented"));
            },
            RecordingFormat::Flac { .. } => {
                return Err(anyhow::anyhow!("FLAC encoding not yet implemented"));
            },
        };
        
        // Create output file
        let file = File::create(&session.current_file_path).await
            .context("Failed to create output file")?;
        
        // Write WAV header if needed
        if matches!(session.config.format, RecordingFormat::Wav) {
            // TODO: Write WAV header
        }
        
        let (_cmd_tx, cmd_rx) = mpsc::channel(100);
        
        // Create recording writer
        let writer = RecordingWriter {
            session: session.clone(),
            encoder,
            file,
            audio_rx,
            command_rx: cmd_rx,
            silence_detector: SilenceDetector::new(
                session.config.silence_threshold_db,
                session.config.silence_duration_sec,
            ),
        };
        
        // Start writer task
        let writer_handle = tokio::spawn(async move {
            if let Err(err) = writer.run().await {
                error!("Recording writer error: {}", err);
            }
        });
        
        // Store writer handle
        self.active_writers.lock().await.insert(session.id.clone(), writer_handle);
        
        Ok(())
    }
    
    /// Save recording configuration preset
    pub async fn save_config(&self, config: RecordingConfig) -> Result<()> {
        self.configs.write().await.insert(config.id.clone(), config);
        Ok(())
    }
    
    /// Get all saved recording configurations
    pub async fn get_configs(&self) -> Vec<RecordingConfig> {
        self.configs.read().await.values().cloned().collect()
    }
    
    /// Get recording history
    pub async fn get_history(&self) -> Vec<RecordingHistoryEntry> {
        self.recording_history.lock().await.clone()
    }
}

impl RecordingWriter {
    /// Main recording loop
    pub async fn run(mut self) -> Result<()> {
        let mut _buffer: Vec<u8> = Vec::new(); // Placeholder for future use
        
        loop {
            tokio::select! {
                // Receive audio samples
                audio_result = self.audio_rx.recv() => {
                    match audio_result {
                        Ok(samples) => {
                            if self.session.is_paused {
                                continue;
                            }
                            
                            // Process audio samples
                            self.process_audio_samples(&samples).await?;
                        },
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Audio stream closed, stopping recording");
                            break;
                        },
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            warn!("Recording lagged behind audio stream");
                            continue;
                        },
                    }
                },
                
                // Receive commands
                cmd_result = self.command_rx.recv() => {
                    match cmd_result {
                        Some(RecordingCommand::Stop) => {
                            info!("Recording stop command received");
                            break;
                        },
                        Some(RecordingCommand::Pause) => {
                            self.session.is_paused = true;
                        },
                        Some(RecordingCommand::Resume) => {
                            self.session.is_paused = false;
                            self.silence_detector.reset();
                        },
                        Some(RecordingCommand::UpdateMetadata(metadata)) => {
                            self.session.config.metadata = metadata;
                        },
                        Some(_) => {
                            // Handle other commands
                        },
                        None => {
                            warn!("Recording command channel closed");
                            break;
                        },
                    }
                },
            }
        }
        
        // Finalize recording
        self.finalize_recording().await?;
        Ok(())
    }
    
    /// Process audio samples
    async fn process_audio_samples(&mut self, samples: &[f32]) -> Result<()> {
        // Update session statistics
        self.session.samples_written += samples.len() as u64;
        self.session.duration_seconds = self.session.samples_written as f64 / 
            (self.session.config.sample_rate as f64 * self.session.config.channels as f64);
        
        // Calculate levels
        if !samples.is_empty() {
            let mid_point = samples.len() / 2;
            let left_samples = &samples[..mid_point];
            let right_samples = &samples[mid_point..];
            
            let left_rms = (left_samples.iter().map(|&x| x * x).sum::<f32>() / left_samples.len() as f32).sqrt();
            let right_rms = (right_samples.iter().map(|&x| x * x).sum::<f32>() / right_samples.len() as f32).sqrt();
            
            self.session.current_levels = (left_rms, right_rms);
            self.session.peak_levels.0 = self.session.peak_levels.0.max(left_rms);
            self.session.peak_levels.1 = self.session.peak_levels.1.max(right_rms);
        }
        
        // Check for silence if auto-stop is enabled
        if self.session.config.auto_stop_on_silence {
            if self.silence_detector.process_samples(samples) {
                info!("Silence detected, stopping recording");
                return Err(anyhow::anyhow!("Recording stopped due to silence"));
            }
        }
        
        // Encode audio
        let encoded_data = self.encoder.encode_samples(samples)?;
        
        // Write to file
        if !encoded_data.is_empty() {
            self.file.write_all(&encoded_data).await?;
            self.session.file_size_bytes += encoded_data.len() as u64;
        }
        
        // Check file size limits
        if let Some(max_size) = self.session.config.max_file_size_mb {
            if self.session.file_size_bytes > (max_size * 1024 * 1024) {
                return Err(anyhow::anyhow!("Recording stopped due to file size limit"));
            }
        }
        
        // Check duration limits
        if let Some(max_duration) = self.session.config.max_duration_minutes {
            if self.session.duration_seconds > (max_duration as f64 * 60.0) {
                return Err(anyhow::anyhow!("Recording stopped due to duration limit"));
            }
        }
        
        Ok(())
    }
    
    /// Finalize the recording
    async fn finalize_recording(&mut self) -> Result<()> {
        // Finalize encoder
        let final_data = self.encoder.finalize()?;
        if !final_data.is_empty() {
            self.file.write_all(&final_data).await?;
        }
        
        // Flush and close file
        self.file.flush().await?;
        
        info!("Recording finalized: {:?}", self.session.current_file_path);
        Ok(())
    }
}

/// Global recording service instance
static RECORDING_SERVICE: tokio::sync::OnceCell<Arc<RecordingService>> = tokio::sync::OnceCell::const_new();

/// Get or initialize the global recording service
pub async fn get_recording_service() -> Arc<RecordingService> {
    RECORDING_SERVICE.get_or_init(|| async {
        Arc::new(RecordingService::new())
    }).await.clone()
}