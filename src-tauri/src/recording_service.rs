use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, mpsc as std_mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{info, error};
use uuid::Uuid;
use lame::Lame;

/// Recording format options - matches frontend TypeScript RecordingFormat
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingFormat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mp3: Option<Mp3Settings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flac: Option<FlacSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wav: Option<WavSettings>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Mp3Settings {
    pub bitrate: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FlacSettings {
    pub compression_level: u8,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WavSettings {
    // Empty for now, can add WAV specific settings later
}

impl Default for RecordingFormat {
    fn default() -> Self {
        RecordingFormat {
            mp3: None,
            flac: None,
            wav: Some(WavSettings {}), // Use WAV as default since MP3 has thread-safety issues
        }
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
pub trait AudioEncoder: Send {
    fn encode_samples(&mut self, samples: &[f32]) -> Result<Vec<u8>>;
    fn finalize(&mut self) -> Result<Vec<u8>>;
    fn estimated_bitrate(&self) -> u32;
    fn as_any(&self) -> &dyn std::any::Any;
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
        
        // fmt chunk - use basic PCM format for maximum compatibility
        header.extend_from_slice(b"fmt ");
        header.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size (16 for basic PCM)
        header.extend_from_slice(&1u16.to_le_bytes());  // Audio format (1 = PCM)
        header.extend_from_slice(&self.channels.to_le_bytes());
        header.extend_from_slice(&self.sample_rate.to_le_bytes());
        
        let bytes_per_sample = match self.bit_depth {
            16 => 2,  // Most compatible - supported by all players
            24 => {
                println!("⚠️ Using 24-bit WAV - may not be compatible with all players (QuickTime, etc.)");
                3
            },
            32 => 4,  // 32-bit float or int
            _ => {
                println!("❌ Unsupported bit depth: {}. Using 16-bit instead.", self.bit_depth);
                2 // Fallback to 16-bit
            },
        };
        
        let byte_rate = self.sample_rate * self.channels as u32 * bytes_per_sample as u32;
        header.extend_from_slice(&byte_rate.to_le_bytes());
        
        let block_align = self.channels * bytes_per_sample;
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
            _ => {
                // Fallback to 16-bit for unsupported bit depths
                println!("⚠️ Encoding: Unsupported bit depth {}, falling back to 16-bit", self.bit_depth);
                for &sample in samples {
                    let sample_i16 = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                    output.extend_from_slice(&sample_i16.to_le_bytes());
                }
            },
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
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Thread-safe MP3 encoder using LAME
/// Uses a dedicated thread for LAME operations to handle Send+Sync requirements
pub struct Mp3Encoder {
    sample_rate: u32,
    channels: u16,
    bitrate: u32,
    quality: u8,
    // Channel to send audio data to the encoder thread
    encoder_tx: mpsc::Sender<Mp3Command>,
    // Channel to receive encoded data from the encoder thread
    encoded_rx: mpsc::Receiver<Vec<u8>>,
    // Handle to the encoder thread
    encoder_handle: Option<tokio::task::JoinHandle<()>>,
}

/// Commands for the MP3 encoder thread
#[derive(Debug)]
enum Mp3Command {
    Encode(Vec<f32>),
    Finalize(std_mpsc::Sender<Vec<u8>>),
    Shutdown,
}

impl Mp3Encoder {
    pub fn new(sample_rate: u32, channels: u16, bitrate: u32, quality: u8) -> Result<Self> {
        let (encoder_tx, mut encoder_rx) = mpsc::channel(100);
        let (encoded_tx, encoded_rx) = mpsc::channel(100);
        
        // Use std::sync channels for synchronous operation within spawn_blocking
        let (std_encoder_tx, std_encoder_rx) = std_mpsc::channel();
        let (std_encoded_tx, std_encoded_rx) = std_mpsc::channel();
        
        // Spawn the encoder thread
        let encoder_handle = tokio::task::spawn_blocking(move || {
            // Initialize LAME encoder
            let mut lame = match Lame::new() {
                Some(lame) => lame,
                None => {
                    error!("Failed to initialize LAME encoder");
                    return;
                }
            };
            
            // Configure LAME
            lame.set_num_channels(channels as u8).expect("Failed to set channels");
            lame.set_sample_rate(sample_rate).expect("Failed to set sample rate");
            lame.set_kilobitrate(bitrate as i32).expect("Failed to set bitrate");
            lame.set_quality(quality as i32).expect("Failed to set quality");
            // Set mode based on channels - let LAME choose the best mode
            if channels == 1 {
                // Mono mode
                lame.set_mode(3).expect("Failed to set mono mode"); // 3 = mono
            } else {
                // Stereo - let LAME choose (joint stereo or stereo)
                lame.set_mode(1).expect("Failed to set stereo mode"); // 1 = joint stereo
            }
            
            if lame.init_params().is_err() {
                error!("Failed to initialize LAME parameters");
                return;
            }
            
            println!("🎵 MP3 encoder initialized: {}Hz, {} channels, {}kbps", 
                sample_rate, channels, bitrate);
            
            // Main encoder loop - synchronous
            while let Ok(command) = std_encoder_rx.recv() {
                match command {
                    Mp3Command::Encode(samples) => {
                        // Convert f32 samples to i16 for LAME
                        let samples_i16: Vec<i16> = samples.iter()
                            .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
                            .collect();
                        
                        // Encode samples
                        let mut mp3_buffer = vec![0u8; samples_i16.len() * 2 + 7200]; // LAME recommended buffer size
                        let encoded_size = if channels == 1 {
                            lame.encode(&samples_i16, &[], &mut mp3_buffer)
                                .unwrap_or_else(|e| {
                                    error!("LAME encoding error: {}", e);
                                    0
                                })
                        } else {
                            // For stereo, split into left/right channels
                            // Audio mixer provides interleaved samples: [L,R,L,R,...]
                            let left: Vec<i16> = samples_i16.iter().step_by(2).copied().collect();
                            let right: Vec<i16> = samples_i16.iter().skip(1).step_by(2).copied().collect();
                            lame.encode(&left, &right, &mut mp3_buffer)
                                .unwrap_or_else(|e| {
                                    error!("LAME encoding error: {}", e);
                                    0
                                })
                        };
                        
                        if encoded_size > 0 {
                            mp3_buffer.truncate(encoded_size);
                            if let Err(_) = std_encoded_tx.send(mp3_buffer) {
                                error!("Failed to send encoded MP3 data");
                            }
                        }
                    },
                    Mp3Command::Finalize(response_tx) => {
                        // Flush remaining data
                        let mut final_buffer = vec![0u8; 7200];
                        let final_size = lame.encode_flush(&mut final_buffer)
                            .unwrap_or_else(|e| {
                                error!("LAME flush error: {}", e);
                                0
                            });
                        
                        if final_size > 0 {
                            final_buffer.truncate(final_size);
                        } else {
                            final_buffer.clear();
                        }
                        
                        let _ = response_tx.send(final_buffer);
                        break; // Exit the loop after finalization
                    },
                    Mp3Command::Shutdown => {
                        break;
                    }
                }
            }
            
            println!("🎵 MP3 encoder thread shutting down");
        });
        
        // Bridge async channels to sync channels
        let encoder_tx_clone = encoder_tx.clone();
        tokio::spawn(async move {
            while let Some(cmd) = encoder_rx.recv().await {
                if std_encoder_tx.send(cmd).is_err() {
                    break; // Encoder thread died
                }
            }
        });
        
        // Bridge sync channel back to async
        let encoded_tx_clone = encoded_tx.clone();
        tokio::spawn(async move {
            while let Ok(data) = std_encoded_rx.recv() {
                if encoded_tx_clone.send(data).await.is_err() {
                    break; // Main thread died
                }
            }
        });
        
        Ok(Self {
            sample_rate,
            channels,
            bitrate,
            quality,
            encoder_tx,
            encoded_rx,
            encoder_handle: Some(encoder_handle),
        })
    }
}

impl AudioEncoder for Mp3Encoder {
    fn encode_samples(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        // Send samples to encoder thread
        if let Err(_) = self.encoder_tx.try_send(Mp3Command::Encode(samples.to_vec())) {
            return Err(anyhow::anyhow!("MP3 encoder thread is busy"));
        }
        
        // Try to receive encoded data (non-blocking)
        match self.encoded_rx.try_recv() {
            Ok(encoded_data) => Ok(encoded_data),
            Err(mpsc::error::TryRecvError::Empty) => Ok(Vec::new()), // No data yet
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Err(anyhow::anyhow!("MP3 encoder thread disconnected"))
            }
        }
    }
    
    fn finalize(&mut self) -> Result<Vec<u8>> {
        let (response_tx, response_rx) = std_mpsc::channel();
        
        // Send finalize command
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.encoder_tx.send(Mp3Command::Finalize(response_tx)).await
                    .map_err(|_| anyhow::anyhow!("Failed to send finalize command to MP3 encoder"))
            })
        })?;
        
        // Wait for final data (with timeout)
        let final_data = response_rx.recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| anyhow::anyhow!("MP3 finalization timeout or error"))?;
        
        // Shutdown encoder thread
        let _ = self.encoder_tx.try_send(Mp3Command::Shutdown);
        if let Some(handle) = self.encoder_handle.take() {
            let _ = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(handle)
            });
        }
        
        Ok(final_data)
    }
    
    fn estimated_bitrate(&self) -> u32 {
        self.bitrate
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Recording writer that handles file I/O and encoding
pub struct RecordingWriter {
    session: RecordingSession,
    file: File,
    audio_rx: broadcast::Receiver<Vec<f32>>,
    silence_detector: SilenceDetector,
    session_update_tx: mpsc::Sender<RecordingSession>, // Channel to send session updates back to service
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
    session_update_rx: Arc<Mutex<Option<mpsc::Receiver<RecordingSession>>>>, // Receiver for session updates
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
            session_update_rx: Arc::new(Mutex::new(None)),
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
        
        // Check for session updates from the recording writer
        if let Some(ref mut update_rx) = *self.session_update_rx.lock().await {
            // Try to receive the latest session update (non-blocking)
            let mut update_count = 0;
            while let Ok(updated_session) = update_rx.try_recv() {
                // Update the current session with latest data
                *self.current_session.lock().await = Some(updated_session);
                update_count += 1;
            }
            if update_count > 0 {
                println!("📈 get_status: Processed {} session updates", update_count);
            }
        } else {
            println!("📈 get_status: No session update receiver available");
        }
        
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
        let extension = if config.format.mp3.is_some() {
            "mp3"
        } else if config.format.flac.is_some() {
            "flac"
        } else if config.format.wav.is_some() {
            "wav"
        } else {
            "wav" // Default to wav
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
        // Create output file
        let file = File::create(&session.current_file_path).await
            .context("Failed to create output file")?;
        
        // Create channel for session updates
        let (session_update_tx, session_update_rx) = mpsc::channel(100);
        
        // Store the receiver for this recording session
        *self.session_update_rx.lock().await = Some(session_update_rx);
        
        // Create recording writer
        let writer = RecordingWriter {
            session: session.clone(),
            file,
            audio_rx,
            silence_detector: SilenceDetector::new(
                session.config.silence_threshold_db,
                session.config.silence_duration_sec,
            ),
            session_update_tx,
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
        // Create encoder based on format (within the task to avoid Send requirement)
        let mut encoder: Box<dyn AudioEncoder> = if let Some(mp3_settings) = &self.session.config.format.mp3 {
            println!("🎵 Creating MP3 encoder: {}kbps", mp3_settings.bitrate);
            Box::new(Mp3Encoder::new(
                self.session.config.sample_rate,
                self.session.config.channels,
                mp3_settings.bitrate,
                2, // Default quality: 0=best, 9=fastest, 2=good balance
            )?)
        } else if self.session.config.format.flac.is_some() {
            return Err(anyhow::anyhow!("FLAC encoding not yet implemented"));
        } else {
            // Default to WAV encoder
            Box::new(WavEncoder::new(
                self.session.config.sample_rate,
                self.session.config.channels,
                self.session.config.bit_depth,
            ))
        };
        
        // Write WAV header if needed
        if self.session.config.format.wav.is_some() {
            let wav_encoder = encoder.as_any().downcast_ref::<WavEncoder>().unwrap();
            let header = wav_encoder.write_header();
            self.file.write_all(&header).await?;
        }
        
        println!("🎧 Recording writer started, waiting for audio data...");
        
        // Check if receiver is still valid
        println!("🔍 Receiver state: active");
        
        // Process any pending messages first!
        loop {
            match self.audio_rx.try_recv() {
                Ok(samples) => {
                    println!("🎵 Processing {} pending samples", samples.len());
                    if !self.session.is_paused {
                        match self.process_audio_samples(&samples, &mut encoder).await {
                            Ok(_) => println!("✅ Processed pending samples successfully"),
                            Err(e) => {
                                println!("❌ Error processing pending samples: {}", e);
                                break;
                            }
                        }
                    }
                },
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                    println!("📭 No more pending samples, starting live processing");
                    break;
                },
                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                    println!("❌ Channel is CLOSED!");
                    return Ok(());
                },
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                    println!("⚠️ Channel lagged by {} messages, continuing", n);
                    continue;
                },
            }
        }
        
        println!("🔄 Starting continuous polling loop...");
        
        // Use a simple loop with non-blocking receives and small sleeps
        loop {
            // Try to receive audio without blocking
            match self.audio_rx.try_recv() {
                Ok(samples) => {
                    // Log every ~1 second instead of every packet to reduce spam
                    static mut SAMPLE_COUNT: u64 = 0;
                    unsafe {
                        SAMPLE_COUNT += 1;
                        if SAMPLE_COUNT % 100 == 0 { // Log every ~100 packets (~1 second)
                            println!("🎵 Recording received packet #{}: {} samples", SAMPLE_COUNT, samples.len());
                        }
                    }
                    
                    if !self.session.is_paused {
                        match self.process_audio_samples(&samples, &mut encoder).await {
                            Ok(_) => {}, // Success, continue
                            Err(e) => {
                                println!("❌ Error processing samples: {}", e);
                                break;
                            }
                        }
                    }
                },
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                    // No data available, sleep briefly and continue
                    tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
                },
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                    println!("⚠️ Recording lagged by {} samples, continuing", n);
                    continue;
                },
                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                    println!("❌ Audio channel closed");
                    break;
                }
            }
            
            // For now, we'll skip command handling to avoid disconnection issues
            // The recording will run until the audio channel closes or we hit an error
            // TODO: Implement proper command handling for pause/resume functionality later
        }
        
        // Finalize recording
        self.finalize_recording(&mut encoder).await?;
        Ok(())
    }
    
    /// Process audio samples
    async fn process_audio_samples(&mut self, samples: &[f32], encoder: &mut Box<dyn AudioEncoder>) -> Result<()> {
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
        let encoded_data = encoder.encode_samples(samples)?;
        
        // Write to file
        if !encoded_data.is_empty() {
            self.file.write_all(&encoded_data).await?;
            self.session.file_size_bytes += encoded_data.len() as u64;
        }
        
        // Send session update to the service (non-blocking) - only every ~10 packets to reduce spam
        static mut UPDATE_COUNT: u64 = 0;
        unsafe {
            UPDATE_COUNT += 1;
            if UPDATE_COUNT % 10 == 0 { // Update UI every ~100ms
                if let Err(_) = self.session_update_tx.try_send(self.session.clone()) {
                    // Channel full - skip this update, UI will get the next one
                } else if UPDATE_COUNT % 100 == 0 { // Log every ~1 second
                    println!("📊 Session update: duration={}s, size={}B, levels=L:{:.1}% R:{:.1}%", 
                        self.session.duration_seconds,
                        self.session.file_size_bytes,
                        self.session.current_levels.0 * 100.0,
                        self.session.current_levels.1 * 100.0
                    );
                }
            }
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
    async fn finalize_recording(&mut self, encoder: &mut Box<dyn AudioEncoder>) -> Result<()> {
        use tokio::io::{AsyncSeekExt, AsyncWriteExt};
        
        // Finalize encoder
        let final_data = encoder.finalize()?;
        if !final_data.is_empty() {
            self.file.write_all(&final_data).await?;
        }
        
        // For WAV files, we need to update the header with correct file sizes
        if self.session.config.format.wav.is_some() {
            println!("🔧 Fixing WAV header with final file size...");
            
            // Get the current file size
            let current_pos = self.file.stream_position().await?;
            let file_size = current_pos as u32;
            
            // Update RIFF chunk size (total file size - 8)
            self.file.seek(tokio::io::SeekFrom::Start(4)).await?;
            self.file.write_all(&(file_size - 8).to_le_bytes()).await?;
            
            // Update data chunk size (file size - header size)
            // WAV header is typically 44 bytes
            let data_size = file_size - 44;
            self.file.seek(tokio::io::SeekFrom::Start(40)).await?;
            self.file.write_all(&data_size.to_le_bytes()).await?;
            
            println!("✅ WAV header updated: file_size={}B, data_size={}B", file_size, data_size);
        }
        
        // Flush and close file
        self.file.flush().await?;
        
        println!("🎯 Recording finalized: {:?}", self.session.current_file_path);
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