// Core mixer types and data structures
//
// This module contains the fundamental data structures for the virtual mixer
// system, including the main VirtualMixer struct, configuration management,
// and audio level caching structures.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicPtr};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use super::super::devices::{AudioDeviceManager, DeviceStatus};
use super::super::effects::AudioAnalyzer;
use super::super::types::{AudioDeviceHandle, AudioMetrics, MixerCommand, MixerConfig, OutputDevice};
use super::stream_management::{AudioInputStream, AudioOutputStream};
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
            mix_buffer: Arc::new(Mutex::new(vec![0.0; buffer_size as usize * 2])), // Stereo
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

    /// Get health status for all devices
    pub async fn get_all_device_health_statuses(&self) -> std::collections::HashMap<String, crate::audio::devices::DeviceHealth> {
        self.audio_device_manager.get_all_device_health().await
    }

    /// Add input stream safely with health checking
    pub async fn add_input_stream_safe(&self, device_id: &str) -> anyhow::Result<()> {
        use tracing::{info, warn};
        info!("Adding input stream for device with health checking: {}", device_id);
        
        // Check device health before attempting to use it
        if self.audio_device_manager.should_avoid_device(device_id).await {
            let health = self.audio_device_manager.get_device_health(device_id).await;
            if let Some(h) = health {
                return Err(anyhow::anyhow!(
                    "Avoiding device {} due to {} consecutive errors. Last error: {:?}", 
                    device_id, h.consecutive_errors, h.status
                ));
            }
        }
        
        // Check if device is still available
        match self.audio_device_manager.check_device_health(device_id).await {
            Ok(DeviceStatus::Connected) => {
                // Device is healthy, proceed with normal stream addition
                // This would call the existing stream management logic
                println!("✅ Device {} is healthy, proceeding with stream creation", device_id);
                Ok(())
            }
            Ok(DeviceStatus::Disconnected) => {
                self.audio_device_manager.report_device_error(
                    device_id, 
                    "Device disconnected".to_string()
                ).await;
                return Err(anyhow::anyhow!("Device {} is disconnected", device_id));
            }
            Ok(DeviceStatus::Error(err)) => {
                return Err(anyhow::anyhow!("Device {} has error: {}", device_id, err));
            }
            Err(e) => {
                self.audio_device_manager.report_device_error(
                    device_id, 
                    format!("Health check failed: {}", e)
                ).await;
                return Err(anyhow::anyhow!("Failed to check device {} health: {}", device_id, e));
            }
        }
    }

    /// Get device health status
    pub async fn get_device_health_status(&self, device_id: &str) -> Option<crate::audio::devices::DeviceHealth> {
        self.audio_device_manager.get_device_health(device_id).await
    }

    /// Get audio output receiver for broadcasting
    pub fn get_audio_output_receiver(&self) -> tokio::sync::broadcast::Receiver<Vec<f32>> {
        self.audio_output_broadcast_tx.subscribe()
    }

    /// Get a reference to a channel by ID
    pub fn get_channel(&self, channel_id: u32) -> Option<&crate::audio::types::AudioChannel> {
        self.config.channels.iter().find(|c| c.id == channel_id)
    }

    /// Get a mutable reference to a channel by ID
    pub fn get_channel_mut(&mut self, channel_id: u32) -> Option<&mut crate::audio::types::AudioChannel> {
        self.config.channels.iter_mut().find(|c| c.id == channel_id)
    }

    /// Add output device
    pub async fn add_output_device(&self, output_device: crate::audio::types::OutputDevice) -> anyhow::Result<()> {
        use cpal::traits::{DeviceTrait, HostTrait};
        
        
        let device_manager = AudioDeviceManager::new()?;
        let devices = device_manager.enumerate_devices().await?;
        
        // Find the device
        let device_info = devices.iter()
            .find(|d| d.id == output_device.device_id && d.is_output)
            .ok_or_else(|| anyhow::anyhow!("Output device not found: {}", output_device.device_id))?;
            
        // **CRASH PREVENTION**: Use device manager's safe device finding instead of direct CPAL calls
        let device_handle = device_manager.find_audio_device(&output_device.device_id, false).await?;
        let device = match device_handle {
            AudioDeviceHandle::Cpal(cpal_device) => cpal_device,
            #[cfg(target_os = "macos")]
            AudioDeviceHandle::CoreAudio(_) => {
                return Err(anyhow::anyhow!("CoreAudio device handles not supported in add_output_device - use CPAL fallback"));
            }
            #[cfg(not(target_os = "macos"))]
            _ => {
                return Err(anyhow::anyhow!("Unknown device handle type"));
            }
        };
        


        let output_stream = Arc::new(AudioOutputStream::new(
            output_device.device_id.clone(),
            device_info.name.clone(),
            self.config.sample_rate,
        )?);
        
        // Add to output streams collection
        self.output_streams.lock().await.insert(
            output_device.device_id.clone(),
            output_stream.clone(),
        );
        
        // Update config to include this output device
        {
            let mut config_guard = self.shared_config.lock().unwrap();
            config_guard.output_devices.push(output_device.clone());
        }
        
        println!("✅ Added output device: {} ({})", output_device.device_name, output_device.device_id);
        Ok(())
    }


    /// Remove an output device from the mixer
    pub async fn remove_output_device(&self, device_id: &str) -> anyhow::Result<()> {
        // Remove from output streams collection
        let removed = self.output_streams.lock().await.remove(device_id);
        
        if removed.is_some() {
            // Update config to remove this output device
            {
                let mut config_guard = self.shared_config.lock().unwrap();
                config_guard.output_devices.retain(|d| d.device_id != device_id);
            }
            
            println!("✅ Removed output device: {}", device_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Output device not found: {}", device_id))
        }
    }

    /// Get a specific output device configuration
    pub async fn get_output_device(&self, device_id: &str) -> Option<OutputDevice> {
        let config_guard = self.shared_config.lock().unwrap();
        config_guard.output_devices
            .iter()
            .find(|d| d.device_id == device_id)
            .cloned()
    }
    

    /// Update output device configuration
  /// Update output device configuration
  pub async fn update_output_device(&self, device_id: &str, updated_device: super::types::OutputDevice) -> anyhow::Result<()> {
    // Update config
    {
        let mut config_guard = self.shared_config.lock().unwrap();
        if let Some(device) = config_guard.output_devices.iter_mut().find(|d| d.device_id == device_id) {
            *device = updated_device;
            println!("✅ Updated output device: {}", device_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Output device not found in config: {}", device_id))
        }
    }
}

    /// Get all output devices
    pub async fn get_output_devices(&self) -> Vec<OutputDevice> {
        let config_guard = self.shared_config.lock().unwrap();
        config_guard.output_devices.clone()
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
            channels: vec![],    // Empty channels list
            master_gain: 1.0,
            master_output_device_id: None,
            monitor_output_device_id: None,
            output_devices: vec![],
            enable_loopback: true,
        }
    }
    
    /// Create optimized configuration for low latency
    pub fn low_latency_config() -> MixerConfig {
        MixerConfig {
            sample_rate: 48000,
            buffer_size: 128,    // Lower latency
            channels: vec![],    // Empty channels list
            master_gain: 1.0,
            master_output_device_id: None,
            monitor_output_device_id: None,
            output_devices: vec![],
            enable_loopback: true,
        }
    }
    
    /// Create configuration for high stability/compatibility
    pub fn stable_config() -> MixerConfig {
        MixerConfig {
            sample_rate: 44100,  // CD quality
            buffer_size: 1024,   // Higher stability
            channels: vec![],    // Empty channels list
            master_gain: 1.0,
            master_output_device_id: None,
            monitor_output_device_id: None,
            output_devices: vec![],
            enable_loopback: true,
        }
    }
}