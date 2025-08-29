// Core mixer types and data structures
//
// This module contains the fundamental data structures for the virtual mixer
// system, including the main VirtualMixer struct, configuration management,
// and audio level caching structures.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicPtr};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use super::super::devices::AudioDeviceManager;
use super::super::effects::AudioAnalyzer;
use super::super::types::{AudioMetrics, MixerCommand, MixerConfig};
use super::transformer::{AudioInputStream, AudioOutputStream};
use super::timing_synchronization::{AudioClock, TimingMetrics};

/// Thread-safe Virtual Mixer with comprehensive audio processing capabilities
/// 
/// # Thread Safety and Locking Order Documentation
/// 
/// This mixer implements a complex audio processing system with multiple threads and shared state.
/// To prevent deadlocks and ensure thread safety, the following locking order MUST be observed:
/// 
/// ## Locking Hierarchy (acquire locks in this order):
/// 1. `active_output_devices` - Track active audio devices (coordination only)
/// 2. `coreaudio_stream` - CoreAudio-specific stream management (macOS only)
/// 3. `output_stream` - High-level output stream wrapper
/// 4. `input_streams` - Input stream management map
/// 5. `channel_levels_cache` / `master_levels_cache` - UI data caches (read frequently)
/// 6. `channel_levels` / `master_levels` - Real-time audio level data
/// 7. `mix_buffer` - Audio processing buffer (high-frequency access)
/// 8. `metrics` - Performance metrics
/// 9. `command_rx` - Command processing channel
#[derive(Debug)]
pub struct VirtualMixer {
    pub config: MixerConfig,
    pub is_running: Arc<AtomicBool>,
    
    // Real-time audio buffers
    pub mix_buffer: Arc<Mutex<Vec<f32>>>,
    
    // Audio processing (placeholder for future sample rate conversion)
    pub sample_rate_converter: Option<()>,
    pub audio_analyzer: AudioAnalyzer,
    
    // Communication channels
    pub command_tx: mpsc::Sender<MixerCommand>,
    pub command_rx: Arc<Mutex<mpsc::Receiver<MixerCommand>>>,
    pub audio_output_tx: mpsc::Sender<Vec<f32>>,
    
    // **STREAMING INTEGRATION**: Broadcast channel for multiple audio output consumers
    pub audio_output_broadcast_tx: tokio::sync::broadcast::Sender<Vec<f32>>,
    
    // Metrics
    pub metrics: Arc<Mutex<AudioMetrics>>,
    
    // Real-time audio level data for VU meters with atomic caching
    pub channel_levels: Arc<Mutex<HashMap<u32, (f32, f32, f32, f32)>>>, // (peak_left, rms_left, peak_right, rms_right)
    pub channel_levels_cache: Arc<Mutex<HashMap<u32, (f32, f32, f32, f32)>>>,
    pub master_levels: Arc<Mutex<(f32, f32, f32, f32)>>,
    pub master_levels_cache: Arc<Mutex<(f32, f32, f32, f32)>>,
    
    // **PRIORITY 5: Audio Clock Synchronization**
    pub audio_clock: Arc<Mutex<AudioClock>>, // Master audio clock for synchronization
    pub timing_metrics: Arc<Mutex<TimingMetrics>>, // Timing performance metrics
    
    // **CRITICAL FIX**: Shared configuration for real-time updates
    pub shared_config: Arc<std::sync::Mutex<MixerConfig>>,
    
    // Audio stream management
    pub audio_device_manager: Arc<AudioDeviceManager>,
    pub input_streams: Arc<Mutex<HashMap<String, Arc<AudioInputStream>>>>,
    pub output_stream: Arc<Mutex<Option<Arc<AudioOutputStream>>>>, // Legacy single output
    pub output_streams: Arc<Mutex<HashMap<String, Arc<AudioOutputStream>>>>, // Multiple outputs
    // Track active output streams by device ID for cleanup (no direct stream storage due to Send/Sync)
    pub active_output_devices: Arc<Mutex<std::collections::HashSet<String>>>,
    
    #[cfg(target_os = "macos")]
    pub coreaudio_stream: Arc<Mutex<Option<crate::audio::devices::coreaudio_stream::CoreAudioOutputStream>>>,
}

impl VirtualMixer {
    /// Create a new virtual mixer with default device manager
    pub async fn new(config: MixerConfig) -> anyhow::Result<Self> {
        let device_manager = Arc::new(AudioDeviceManager::new()?);
        Self::new_with_device_manager(config, device_manager).await
    }

    /// Create a new virtual mixer with provided device manager
    pub async fn new_with_device_manager(
        config: MixerConfig, 
        device_manager: Arc<AudioDeviceManager>
    ) -> anyhow::Result<Self> {
        // Validate configuration
        super::validation::validate_config(&config)?;
        
        let (command_tx, command_rx) = mpsc::channel(100);
        let (audio_output_tx, _) = mpsc::channel(1000);
        let (audio_output_broadcast_tx, _) = tokio::sync::broadcast::channel(100);
        
        let buffer_size = config.buffer_size;
        let sample_rate = config.sample_rate;
        
        Ok(Self {
            config: config.clone(),
            is_running: Arc::new(AtomicBool::new(false)),
            mix_buffer: Arc::new(Mutex::new(vec![0.0; buffer_size * 2])), // Stereo
            sample_rate_converter: None,
            audio_analyzer: AudioAnalyzer::new(sample_rate),
            command_tx,
            command_rx: Arc::new(Mutex::new(command_rx)),
            audio_output_tx,
            audio_output_broadcast_tx,
            metrics: Arc::new(Mutex::new(AudioMetrics::default())),
            channel_levels: Arc::new(Mutex::new(HashMap::new())),
            channel_levels_cache: Arc::new(Mutex::new(HashMap::new())),
            master_levels: Arc::new(Mutex::new((0.0, 0.0, 0.0, 0.0))),
            master_levels_cache: Arc::new(Mutex::new((0.0, 0.0, 0.0, 0.0))),
            audio_clock: Arc::new(Mutex::new(AudioClock::new(sample_rate, buffer_size as u32))),
            timing_metrics: Arc::new(Mutex::new(TimingMetrics::new())),
            shared_config: Arc::new(std::sync::Mutex::new(config)),
            audio_device_manager: device_manager,
            input_streams: Arc::new(Mutex::new(HashMap::new())),
            output_stream: Arc::new(Mutex::new(None)),
            output_streams: Arc::new(Mutex::new(HashMap::new())),
            active_output_devices: Arc::new(Mutex::new(std::collections::HashSet::new())),
            
            #[cfg(target_os = "macos")]
            coreaudio_stream: Arc::new(Mutex::new(None)),
        })
    }
}

/// Configuration utilities for mixer setup
pub struct MixerConfigUtils;

impl MixerConfigUtils {
    /// Get default mixer configuration for the platform
    pub fn default_config() -> MixerConfig {
        MixerConfig {
            sample_rate: 48000,  // Professional standard
            buffer_size: 512,    // Balance of latency and stability
            channels: 2,         // Stereo
        }
    }
    
    /// Create optimized configuration for low latency
    pub fn low_latency_config() -> MixerConfig {
        MixerConfig {
            sample_rate: 48000,
            buffer_size: 128,    // Lower latency
            channels: 2,
        }
    }
    
    /// Create configuration for high stability/compatibility
    pub fn stable_config() -> MixerConfig {
        MixerConfig {
            sample_rate: 44100,  // CD quality
            buffer_size: 1024,   // Higher stability
            channels: 2,
        }
    }
}