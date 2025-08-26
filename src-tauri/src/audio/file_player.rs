use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;
use serde::{Serialize, Deserialize};

use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
// Removed unused import
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

/// Represents a single track in the queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedTrack {
    pub id: String,
    pub file_path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<Duration>,
    pub file_size: u64,
    pub added_at: chrono::DateTime<chrono::Utc>,
}

/// Current playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// Playback mode settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackMode {
    pub repeat_mode: RepeatMode,
    pub shuffle: bool,
    pub crossfade_duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatMode {
    None,
    Track,
    Queue,
}

/// Current playback status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackStatus {
    pub state: PlaybackState,
    pub current_track: Option<QueuedTrack>,
    pub position: Duration,
    pub volume: f32, // 0.0 to 1.0
    pub queue_length: usize,
    pub mode: PlaybackMode,
}

/// Audio file player that decodes files and provides audio samples
pub struct AudioFilePlayer {
    // Queue management
    queue: Arc<Mutex<VecDeque<QueuedTrack>>>,
    current_track_index: Arc<Mutex<Option<usize>>>,
    
    // Playback state
    state: Arc<Mutex<PlaybackState>>,
    volume: Arc<Mutex<f32>>,
    position: Arc<Mutex<Duration>>,
    mode: Arc<Mutex<PlaybackMode>>,
    
    // Audio processing
    sample_rate: u32,
    channels: u16,
    
    // Output broadcast for mixer integration
    audio_tx: broadcast::Sender<Vec<f32>>,
    
    // Current decoder and format reader
    current_decoder: Arc<Mutex<Option<Box<dyn Decoder>>>>,
    current_reader: Arc<Mutex<Option<Box<dyn FormatReader>>>>,
    
    // Resampler for format conversion
    resampler: Arc<Mutex<Option<SincFixedIn<f32>>>>,
}

impl AudioFilePlayer {
    /// Create a new audio file player
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let (audio_tx, _) = broadcast::channel(1024);
        
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            current_track_index: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(PlaybackState::Stopped)),
            volume: Arc::new(Mutex::new(1.0)),
            position: Arc::new(Mutex::new(Duration::ZERO)),
            mode: Arc::new(Mutex::new(PlaybackMode {
                repeat_mode: RepeatMode::None,
                shuffle: false,
                crossfade_duration: Duration::from_millis(500),
            })),
            sample_rate,
            channels,
            audio_tx,
            current_decoder: Arc::new(Mutex::new(None)),
            current_reader: Arc::new(Mutex::new(None)),
            resampler: Arc::new(Mutex::new(None)),
        }
    }
    
    /// Add a track to the queue
    pub async fn add_track<P: AsRef<Path>>(&self, file_path: P) -> Result<String> {
        let path = file_path.as_ref().to_path_buf();
        
        // Validate file exists and get metadata
        let metadata = tokio::fs::metadata(&path)
            .await
            .context("Failed to read file metadata")?;
        
        if !metadata.is_file() {
            return Err(anyhow::anyhow!("Path is not a file"));
        }
        
        // Extract audio metadata using symphonia
        let (title, artist, album, duration) = self.extract_metadata(&path).await?;
        
        let track = QueuedTrack {
            id: uuid::Uuid::new_v4().to_string(),
            file_path: path,
            title,
            artist,
            album,
            duration,
            file_size: metadata.len(),
            added_at: chrono::Utc::now(),
        };
        
        let track_id = track.id.clone();
        
        // Add to queue
        {
            let mut queue = self.queue.lock().unwrap();
            queue.push_back(track);
        }
        
        println!("📀 Added track to queue: {:?}", track_id);
        Ok(track_id)
    }
    
    /// Remove a track from the queue
    pub fn remove_track(&self, track_id: &str) -> Result<()> {
        let mut queue = self.queue.lock().unwrap();
        let original_len = queue.len();
        
        queue.retain(|track| track.id != track_id);
        
        if queue.len() == original_len {
            return Err(anyhow::anyhow!("Track not found in queue"));
        }
        
        println!("🗑️ Removed track from queue: {}", track_id);
        Ok(())
    }
    
    /// Start playback
    pub async fn play(&self) -> Result<()> {
        let current_state = {
            let state = self.state.lock().unwrap();
            *state
        };
        
        match current_state {
            PlaybackState::Stopped => {
                // Start playing first track in queue
                self.load_next_track().await?;
                let mut state = self.state.lock().unwrap();
                *state = PlaybackState::Playing;
            }
            PlaybackState::Paused => {
                // Resume playback
                let mut state = self.state.lock().unwrap();
                *state = PlaybackState::Playing;
            }
            PlaybackState::Playing => {
                // Already playing
                return Ok(());
            }
        }
        
        println!("▶️ Started playback");
        Ok(())
    }
    
    /// Pause playback
    pub fn pause(&self) {
        let mut state = self.state.lock().unwrap();
        if *state == PlaybackState::Playing {
            *state = PlaybackState::Paused;
            println!("⏸️ Paused playback");
        }
    }
    
    /// Stop playback
    pub fn stop(&self) {
        {
            let mut state = self.state.lock().unwrap();
            *state = PlaybackState::Stopped;
        }
        
        // Reset position
        {
            let mut position = self.position.lock().unwrap();
            *position = Duration::ZERO;
        }
        
        // Clear current decoder
        {
            let mut decoder = self.current_decoder.lock().unwrap();
            *decoder = None;
        }
        
        {
            let mut reader = self.current_reader.lock().unwrap();
            *reader = None;
        }
        
        println!("⏹️ Stopped playback");
    }
    
    /// Skip to next track
    pub async fn skip_next(&self) -> Result<()> {
        self.load_next_track().await?;
        println!("⏭️ Skipped to next track");
        Ok(())
    }
    
    /// Skip to previous track
    pub async fn skip_previous(&self) -> Result<()> {
        // For now, just restart current track
        // TODO: Implement proper previous track logic
        {
            let mut position = self.position.lock().unwrap();
            *position = Duration::ZERO;
        }
        println!("⏮️ Skipped to previous track");
        Ok(())
    }
    
    /// Set playback volume (0.0 to 1.0)
    pub fn set_volume(&self, volume: f32) {
        let mut vol = self.volume.lock().unwrap();
        *vol = volume.clamp(0.0, 1.0);
    }
    
    /// Get current playback status
    pub fn get_status(&self) -> PlaybackStatus {
        let queue = self.queue.lock().unwrap();
        let state = *self.state.lock().unwrap();
        let position = *self.position.lock().unwrap();
        let volume = *self.volume.lock().unwrap();
        let mode = self.mode.lock().unwrap().clone();
        
        let current_track = if let Some(index) = *self.current_track_index.lock().unwrap() {
            queue.get(index).cloned()
        } else {
            None
        };
        
        PlaybackStatus {
            state,
            current_track,
            position,
            volume,
            queue_length: queue.len(),
            mode,
        }
    }
    
    /// Get queue contents
    pub fn get_queue(&self) -> Vec<QueuedTrack> {
        self.queue.lock().unwrap().iter().cloned().collect()
    }
    
    /// Get audio output receiver for mixer integration
    pub fn get_audio_receiver(&self) -> broadcast::Receiver<Vec<f32>> {
        self.audio_tx.subscribe()
    }
    
    /// Load and start playing the next track
    async fn load_next_track(&self) -> Result<()> {
        let track = {
            let queue = self.queue.lock().unwrap();
            
            if queue.is_empty() {
                return Err(anyhow::anyhow!("Queue is empty"));
            }
            
            // For now, just play first track
            // TODO: Implement proper next track logic with shuffle/repeat
            queue.front().unwrap().clone()
        };
        
        self.load_track(&track).await?;
        
        let mut current_index = self.current_track_index.lock().unwrap();
        *current_index = Some(0);
        
        Ok(())
    }
    
    /// Load a specific track for playback
    async fn load_track(&self, track: &QueuedTrack) -> Result<()> {
        println!("🎵 Loading track: {:?}", track.file_path);
        
        // Open the file
        let file = std::fs::File::open(&track.file_path)
            .context("Failed to open audio file")?;
        
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        
        // Create a probe hint using the file extension
        let mut hint = Hint::new();
        if let Some(extension) = track.file_path.extension() {
            if let Some(extension_str) = extension.to_str() {
                hint.with_extension(extension_str);
            }
        }
        
        // Use the default options for metadata and format readers
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();
        
        // Probe the media source
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .context("Unsupported format")?;
        
        // Get the instantiated format reader
        let mut format = probed.format;
        
        // Find the first audio track with a known (decodeable) codec
        let track_info = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .context("No supported audio tracks")?;
        
        let track_id = track_info.id;
        let track_info_cloned = track_info.clone();
        
        // Use the default options for the decoder
        let dec_opts: DecoderOptions = Default::default();
        
        // Create a decoder for the track
        let decoder = symphonia::default::get_codecs()
            .make(&track_info.codec_params, &dec_opts)
            .context("Unsupported codec")?;
        
        // Store the decoder and reader
        {
            let mut current_decoder = self.current_decoder.lock().unwrap();
            *current_decoder = Some(decoder);
        }
        
        {
            let mut current_reader = self.current_reader.lock().unwrap();
            *current_reader = Some(format);
        }
        
        // Initialize resampler if needed
        let input_sample_rate = track_info_cloned.codec_params.sample_rate.unwrap_or(44100);
        let input_channels = track_info_cloned.codec_params.channels.unwrap().count();
        
        if input_sample_rate != self.sample_rate || input_channels != self.channels as usize {
            let params = SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                oversampling_factor: 256,
                interpolation: SincInterpolationType::Linear,
                window: WindowFunction::BlackmanHarris2,
            };
            
            let resampler = SincFixedIn::<f32>::new(
                self.sample_rate as f64 / input_sample_rate as f64,
                2.0, // Max relative change in sample rate
                params,
                1024, // Chunk size
                input_channels,
            ).context("Failed to create resampler")?;
            
            let mut current_resampler = self.resampler.lock().unwrap();
            *current_resampler = Some(resampler);
        }
        
        println!("✅ Track loaded successfully");
        Ok(())
    }
    
    /// Extract metadata from an audio file
    async fn extract_metadata(&self, path: &Path) -> Result<(Option<String>, Option<String>, Option<String>, Option<Duration>)> {
        // For now, return None for all metadata
        // TODO: Implement proper metadata extraction using symphonia
        Ok((None, None, None, None))
    }
}

/// Represents a virtual audio device that streams from the file player
pub struct FilePlayerDevice {
    player: Arc<AudioFilePlayer>,
    device_id: String,
    device_name: String,
}

impl FilePlayerDevice {
    pub fn new(device_name: String, sample_rate: u32, channels: u16) -> Self {
        let device_id = format!("file_player_{}", uuid::Uuid::new_v4());
        let player = Arc::new(AudioFilePlayer::new(sample_rate, channels));
        
        Self {
            player,
            device_id,
            device_name,
        }
    }
    
    pub fn get_device_id(&self) -> &str {
        &self.device_id
    }
    
    pub fn get_device_name(&self) -> &str {
        &self.device_name
    }
    
    pub fn get_player(&self) -> Arc<AudioFilePlayer> {
        self.player.clone()
    }
}