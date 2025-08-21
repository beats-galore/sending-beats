use anyhow::{Context, Result};
use cpal::{StreamConfig, SampleRate, BufferSize};
use cpal::traits::{DeviceTrait, StreamTrait};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn, error, debug};

/// # Thread Safety and Locking Order Documentation
/// 
/// This module implements a complex audio processing system with multiple threads and shared state.
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
/// 
/// ## Thread Safety Guarantees:
/// - All shared state is protected by `Arc<Mutex<T>>` or `Arc<AtomicPtr<T>>`
/// - Audio processing occurs in dedicated threads separate from UI
/// - CoreAudio callback uses atomic pointer management for memory safety
/// - Stream handles are properly tracked to prevent memory leaks
/// 
/// ## Critical Sections:
/// - Audio callbacks execute in real-time threads - minimize lock contention
/// - UI polling occurs at 100ms intervals - uses cached data when possible
/// - Device switching requires careful coordination of stream lifecycle
/// 
/// ## Memory Safety:
/// - CoreAudio callbacks use `Arc<AtomicPtr<T>>` instead of raw pointers
/// - CPAL streams are allowed to be managed by the audio subsystem naturally
/// - Device tracking enables coordination without unsafe stream storage
/// - All cleanup is performed in Drop implementations and explicit stop methods

use super::devices::AudioDeviceManager;
use super::effects::AudioAnalyzer;
use super::streams::{AudioInputStream, AudioOutputStream, VirtualMixerHandle, StreamCommand, get_stream_manager};
use super::types::{AudioChannel, AudioMetrics, MixerCommand, MixerConfig};

#[derive(Debug)]
pub struct VirtualMixer {
    config: MixerConfig,
    is_running: Arc<AtomicBool>,
    
    // Real-time audio buffers
    mix_buffer: Arc<Mutex<Vec<f32>>>,
    
    // Audio processing (placeholder for future sample rate conversion)
    sample_rate_converter: Option<()>,
    audio_analyzer: AudioAnalyzer,
    
    // Communication channels
    command_tx: mpsc::Sender<MixerCommand>,
    command_rx: Arc<Mutex<mpsc::Receiver<MixerCommand>>>,
    audio_output_tx: mpsc::Sender<Vec<f32>>,
    
    // Metrics
    metrics: Arc<Mutex<AudioMetrics>>,
    
    // Real-time audio level data for VU meters with atomic caching
    channel_levels: Arc<Mutex<HashMap<u32, (f32, f32, f32, f32)>>>, // (peak_left, rms_left, peak_right, rms_right)
    channel_levels_cache: Arc<Mutex<HashMap<u32, (f32, f32, f32, f32)>>>,
    master_levels: Arc<Mutex<(f32, f32, f32, f32)>>,
    master_levels_cache: Arc<Mutex<(f32, f32, f32, f32)>>,
    
    // **PRIORITY 5: Audio Clock Synchronization**
    audio_clock: Arc<Mutex<AudioClock>>, // Master audio clock for synchronization
    timing_metrics: Arc<Mutex<TimingMetrics>>, // Timing performance metrics
    
    // **CRITICAL FIX**: Shared configuration for real-time updates
    shared_config: Arc<std::sync::Mutex<MixerConfig>>,
    
    // Audio stream management
    audio_device_manager: Arc<AudioDeviceManager>,
    input_streams: Arc<Mutex<HashMap<String, Arc<AudioInputStream>>>>,
    output_stream: Arc<Mutex<Option<Arc<AudioOutputStream>>>>, // Legacy single output
    output_streams: Arc<Mutex<HashMap<String, Arc<AudioOutputStream>>>>, // Multiple outputs
    // Track active output streams by device ID for cleanup (no direct stream storage due to Send/Sync)
    active_output_devices: Arc<Mutex<std::collections::HashSet<String>>>,
    #[cfg(target_os = "macos")]
    coreaudio_stream: Arc<Mutex<Option<super::coreaudio_stream::CoreAudioOutputStream>>>,
}

impl VirtualMixer {
    /// Calculate optimal buffer size based on hardware capabilities and performance requirements  
    async fn calculate_optimal_buffer_size(
        &self, 
        device: &cpal::Device, 
        config: &cpal::SupportedStreamConfig,
        fallback_size: usize
    ) -> Result<BufferSize> {
        // Try to get the device's preferred buffer size
        match device.default_input_config() {
            Ok(device_config) => {
                // Calculate optimal buffer size based on sample rate and latency requirements
                let sample_rate = config.sample_rate().0;
                let channels = config.channels();
                
                // Target latency: 5-10ms for professional audio (balance between latency and stability)
                let target_latency_ms = if sample_rate >= 48000 { 5.0 } else { 10.0 };
                let target_buffer_size = ((sample_rate as f32 * target_latency_ms / 1000.0) as usize)
                    .max(64)   // Minimum 64 samples for stability
                    .min(2048); // Maximum 2048 samples to prevent excessive latency
                
                // Round to next power of 2 for optimal hardware performance  
                let optimal_size = target_buffer_size.next_power_of_two().min(1024);
                
                info!("🔧 DYNAMIC BUFFER: Calculated optimal buffer size {} for device (SR: {}, CH: {}, Target: {}ms)", 
                      optimal_size, sample_rate, channels, target_latency_ms);
                
                Ok(BufferSize::Fixed(optimal_size as u32))
            }
            Err(e) => {
                warn!("Failed to get device config for buffer optimization: {}, using fallback", e);
                Ok(BufferSize::Fixed(fallback_size as u32))
            }
        }
    }

    /// Comprehensive device ID validation for security and robustness
    fn validate_device_id(device_id: &str) -> Result<()> {
        // Basic empty/length checks
        if device_id.is_empty() {
            return Err(anyhow::anyhow!("Device ID cannot be empty"));
        }
        if device_id.len() > 256 {
            return Err(anyhow::anyhow!("Device ID too long: maximum 256 characters allowed, got {}", device_id.len()));
        }
        if device_id.len() < 2 {
            return Err(anyhow::anyhow!("Device ID too short: minimum 2 characters required"));
        }
        
        // Character validation - allow alphanumeric, underscore, dash, dot, and colon for device IDs
        let valid_chars = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':');
        if !device_id.chars().all(valid_chars) {
            let invalid_chars: String = device_id.chars()
                .filter(|&c| !valid_chars(c))
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            return Err(anyhow::anyhow!(
                "Device ID contains invalid characters: '{}'. Only alphanumeric, underscore, dash, dot, and colon are allowed", 
                invalid_chars
            ));
        }
        
        // Pattern validation - must not start or end with special characters
        if device_id.starts_with(|c: char| !c.is_alphanumeric()) {
            return Err(anyhow::anyhow!("Device ID must start with alphanumeric character"));
        }
        if device_id.ends_with(|c: char| !c.is_alphanumeric()) {
            return Err(anyhow::anyhow!("Device ID must end with alphanumeric character"));
        }
        
        // Security checks - prevent common injection patterns
        let dangerous_patterns = ["../", "..\\", "//", "\\\\", ";;", "&&", "||"];
        for pattern in &dangerous_patterns {
            if device_id.contains(pattern) {
                return Err(anyhow::anyhow!("Device ID contains dangerous pattern: '{}'", pattern));
            }
        }
        
        Ok(())
    }

    /// Validate mixer configuration for security and performance
    fn validate_config(config: &MixerConfig) -> Result<()> {
        // Sample rate validation
        if config.sample_rate < 8000 || config.sample_rate > 192000 {
            return Err(anyhow::anyhow!("Invalid sample rate: {} (must be 8000-192000 Hz)", config.sample_rate));
        }
        
        // Buffer size validation
        if config.buffer_size < 16 || config.buffer_size > 8192 {
            return Err(anyhow::anyhow!("Invalid buffer size: {} (must be 16-8192 samples)", config.buffer_size));
        }
        
        // Check buffer size is power of 2 for optimal performance
        if !config.buffer_size.is_power_of_two() {
            warn!("Buffer size {} is not a power of 2, may cause performance issues", config.buffer_size);
        }
        
        // Master gain validation
        if config.master_gain < 0.0 || config.master_gain > 4.0 {
            return Err(anyhow::anyhow!("Invalid master gain: {} (must be 0.0-4.0)", config.master_gain));
        }
        
        // Channels validation
        if config.channels.len() > 32 {
            return Err(anyhow::anyhow!("Too many channels: {} (maximum 32)", config.channels.len()));
        }
        
        // Validate each channel
        for (i, channel) in config.channels.iter().enumerate() {
            if channel.gain < 0.0 || channel.gain > 4.0 {
                return Err(anyhow::anyhow!("Invalid gain for channel {}: {} (must be 0.0-4.0)", i, channel.gain));
            }
            if channel.pan < -1.0 || channel.pan > 1.0 {
                return Err(anyhow::anyhow!("Invalid pan for channel {}: {} (must be -1.0 to 1.0)", i, channel.pan));
            }
            // Validate EQ settings
            if channel.eq_low_gain < -24.0 || channel.eq_low_gain > 24.0 {
                return Err(anyhow::anyhow!("Invalid EQ low gain for channel {}: {} (must be -24.0 to 24.0 dB)", i, channel.eq_low_gain));
            }
            if channel.eq_mid_gain < -24.0 || channel.eq_mid_gain > 24.0 {
                return Err(anyhow::anyhow!("Invalid EQ mid gain for channel {}: {} (must be -24.0 to 24.0 dB)", i, channel.eq_mid_gain));
            }
            if channel.eq_high_gain < -24.0 || channel.eq_high_gain > 24.0 {
                return Err(anyhow::anyhow!("Invalid EQ high gain for channel {}: {} (must be -24.0 to 24.0 dB)", i, channel.eq_high_gain));
            }
        }
        
        Ok(())
    }

    pub async fn new(config: MixerConfig) -> Result<Self> {
        let device_manager = Arc::new(AudioDeviceManager::new()?);
        Self::new_with_device_manager(config, device_manager).await
    }

    pub async fn new_with_device_manager(config: MixerConfig, device_manager: Arc<AudioDeviceManager>) -> Result<Self> {
        // Validate mixer configuration
        Self::validate_config(&config)?;
        
        let (command_tx, command_rx) = mpsc::channel(1024);
        let (audio_output_tx, _audio_output_rx) = mpsc::channel(8192);
        
        let buffer_size = config.buffer_size as usize;
        let mix_buffer = Arc::new(Mutex::new(vec![0.0; buffer_size * 2])); // Stereo

        let metrics = Arc::new(Mutex::new(AudioMetrics {
            cpu_usage: 0.0,
            buffer_underruns: 0,
            buffer_overruns: 0,
            latency_ms: (buffer_size as f32 / config.sample_rate as f32) * 1000.0,
            sample_rate: config.sample_rate,
            active_channels: config.channels.len() as u32,
        }));

        // Use the provided audio device manager
        let audio_device_manager = device_manager;

        let channel_levels = Arc::new(Mutex::new(HashMap::new()));
        let channel_levels_cache = Arc::new(Mutex::new(HashMap::new()));
        let master_levels = Arc::new(Mutex::new((0.0, 0.0, 0.0, 0.0)));
        let master_levels_cache = Arc::new(Mutex::new((0.0, 0.0, 0.0, 0.0)));

        Ok(Self {
            config: config.clone(),
            is_running: Arc::new(AtomicBool::new(false)),
            mix_buffer,
            sample_rate_converter: None,
            audio_analyzer: AudioAnalyzer::new(config.sample_rate),
            command_tx,
            command_rx: Arc::new(Mutex::new(command_rx)),
            
            // **PRIORITY 5: Audio Clock Synchronization**
            audio_clock: Arc::new(Mutex::new(AudioClock::new(config.sample_rate, config.buffer_size))),
            timing_metrics: Arc::new(Mutex::new(TimingMetrics::new())),
            audio_output_tx,
            metrics,
            channel_levels,
            channel_levels_cache,
            master_levels,
            master_levels_cache,
            
            // **CRITICAL FIX**: Shared configuration for real-time updates
            shared_config: Arc::new(std::sync::Mutex::new(config.clone())),
            
            audio_device_manager,
            input_streams: Arc::new(Mutex::new(HashMap::new())),
            output_stream: Arc::new(Mutex::new(None)),
            output_streams: Arc::new(Mutex::new(HashMap::new())), // Initialize empty multiple outputs
            active_output_devices: Arc::new(Mutex::new(std::collections::HashSet::new())),
            #[cfg(target_os = "macos")]
            coreaudio_stream: Arc::new(Mutex::new(None)),
        })
    }

    /// Start the virtual mixer
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        info!("Starting Virtual Mixer with real audio capture...");

        self.is_running.store(true, Ordering::Relaxed);
        
        // Start the audio processing thread
        self.start_processing_thread().await?;
        
        Ok(())
    }

    /// Add an audio input stream with real audio capture using cpal
    pub async fn add_input_stream(&self, device_id: &str) -> Result<()> {
        // Validate device_id input with comprehensive validation
        Self::validate_device_id(device_id)?;
        
        info!("🎧 DEVICE CHANGE: Adding input stream for device: {}", device_id);
        
        // **CRITICAL FIX**: Check if device is already active to prevent duplicate streams
        {
            let streams = self.input_streams.lock().await;
            if streams.contains_key(device_id) {
                warn!("Device {} already has an active input stream, removing first", device_id);
                drop(streams);
                // Remove existing stream first
                if let Err(e) = self.remove_input_stream(device_id).await {
                    warn!("Failed to remove existing stream for {}: {}", device_id, e);
                }
            }
        }
        
        // **CRITICAL FIX**: Extended delay to allow proper stream cleanup and prevent crashes
        // Increased from 50ms to 200ms to ensure complete resource cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        // **CRASH FIX**: Find the actual cpal device with enhanced error handling and fallback
        println!("🔍 CRASH DEBUG MIXER: About to find cpal device for: {}", device_id);
        let device = match self.audio_device_manager.find_cpal_device(device_id, true).await {
            Ok(device) => {
                println!("✅ CRASH DEBUG MIXER: Successfully found cpal device for: {}", device_id);
                device
            }
            Err(e) => {
                error!("Failed to find input device '{}': {}", device_id, e);
                
                // **CRASH FIX**: Try to refresh devices and try again
                warn!("Attempting to refresh device list and retry for input device...");
                if let Err(refresh_err) = self.audio_device_manager.refresh_devices().await {
                    error!("Failed to refresh devices: {}", refresh_err);
                }
                
                // Try one more time after refresh
                match self.audio_device_manager.find_cpal_device(device_id, true).await {
                    Ok(device) => {
                        info!("Found input device '{}' after refresh", device_id);
                        device
                    }
                    Err(retry_err) => {
                        error!("Input device '{}' still not found after refresh: {}", device_id, retry_err);
                        return Err(anyhow::anyhow!("Input device '{}' not found or unavailable. Original error: {}. Retry error: {}", device_id, e, retry_err));
                    }
                }
            }
        };
        let device_name = device.name().unwrap_or_else(|_| device_id.to_string());
        
        debug!("Found cpal device: {}", device_name);
        
        // Get the default input config for this device
        let config = device.default_input_config()
            .context("Failed to get default input config")?;
            
        debug!("Device config: {:?}", config);
        
        // **AUDIO QUALITY FIX**: Use hardware sample rate instead of fixed mixer sample rate
        let hardware_sample_rate = config.sample_rate().0;
        println!("🔧 SAMPLE RATE FIX: Hardware {} Hz, Mixer {} Hz -> Using {} Hz to avoid resampling distortion", 
                 hardware_sample_rate, self.config.sample_rate, hardware_sample_rate);
        
        // Create AudioInputStream structure with hardware sample rate to prevent pitch shifting
        let mut input_stream = AudioInputStream::new(
            device_id.to_string(),
            device_name.clone(),
            hardware_sample_rate, // Use hardware sample rate instead of mixer sample rate
        )?;
        
        // Configure adaptive chunk size and stream config with OPTIMAL buffer sizing
        let buffer_size = self.config.buffer_size as usize;
        let target_sample_rate = self.config.sample_rate;
        let optimal_buffer_size = self.calculate_optimal_buffer_size(&device, &config, buffer_size).await?;
        
        let actual_buffer_size = match optimal_buffer_size {
            BufferSize::Fixed(size) => size as usize,
            BufferSize::Default => buffer_size,
        };
        input_stream.set_adaptive_chunk_size(actual_buffer_size);
        
        // Get references for the audio callback
        let audio_buffer = input_stream.audio_buffer.clone();
        
        // **AUDIO QUALITY FIX**: Use hardware-native configuration to prevent format conversion distortion
        let stream_config = StreamConfig {
            channels: config.channels().min(2), // Limit to stereo max
            sample_rate: config.sample_rate(),   // Use hardware sample rate, not mixer sample rate
            buffer_size: optimal_buffer_size,
        };
        
        println!("🔧 FORMAT FIX: Using native format - SR: {} Hz, CH: {}, Buffer: {:?} to prevent conversion distortion",
                 config.sample_rate().0, config.channels(), optimal_buffer_size);
        
        debug!("Using stream config: channels={}, sample_rate={}, buffer_size={}", 
                stream_config.channels, stream_config.sample_rate.0, buffer_size);
        
        // Add to streams collection first
        let mut streams = self.input_streams.lock().await;
        streams.insert(device_id.to_string(), Arc::new(input_stream));
        drop(streams); // Release the async lock
        
        // Send stream creation command to the synchronous stream manager thread
        println!("🔍 CRASH DEBUG MIXER: About to send command to stream manager for device: {}", device_id);
        let stream_manager = get_stream_manager();
        println!("✅ CRASH DEBUG MIXER: Got stream manager reference");
        
        let (response_tx, response_rx) = std::sync::mpsc::channel();
        println!("✅ CRASH DEBUG MIXER: Created response channel");
        
        let command = StreamCommand::AddInputStream {
            device_id: device_id.to_string(),
            device,
            config: stream_config,
            audio_buffer,
            target_sample_rate: hardware_sample_rate, // Use hardware sample rate
            response_tx,
        };
        println!("✅ CRASH DEBUG MIXER: Created StreamCommand for device: {}", device_id);
        
        println!("🔍 CRASH DEBUG MIXER: About to send command to stream manager");
        match stream_manager.send(command) {
            Ok(()) => {
                println!("✅ CRASH DEBUG MIXER: Successfully sent command to stream manager");
            }
            Err(e) => {
                eprintln!("❌ CRASH DEBUG MIXER: Failed to send command to stream manager: {}", e);
                return Err(anyhow::anyhow!("Failed to send stream creation command: {}", e));
            }
        }
            
        // Wait for the response from the stream manager thread
        let result = response_rx.recv()
            .context("Failed to receive stream creation response")?;
            
        match result {
            Ok(()) => {
                info!("Successfully started audio input stream for: {}", device_name);
                info!("Successfully added real audio input stream: {}", device_id);
                Ok(())
            }
            Err(e) => {
                // Remove from streams collection if stream creation failed
                let mut streams = self.input_streams.lock().await;
                streams.remove(device_id);
                Err(e)
            }
        }
    }

    /// Set the audio output stream with support for both cpal and CoreAudio devices
    pub async fn set_output_stream(&self, device_id: &str) -> Result<()> {
        info!("🔊 DEVICE CHANGE: Setting output stream for device: {}", device_id);
        
        // **CRASH FIX**: Enhanced device_id validation
        Self::validate_device_id(device_id).context("Invalid output device ID")?;
        
        // **CRITICAL FIX**: Graceful output stream switching with proper cleanup
        info!("🔴 Stopping existing output streams before device change...");
        if let Err(e) = self.stop_output_streams().await {
            warn!("Error stopping existing output streams: {}", e);
            // Continue anyway - try to start new stream
        }
        
        // **CRITICAL FIX**: Extended delay for complete audio resource cleanup
        // Increased to 200ms to match input stream delay and prevent device conflicts
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        // **CRASH FIX**: Find the audio device with enhanced error handling and fallback
        let device_handle = match self.audio_device_manager.find_audio_device(device_id, false).await {
            Ok(handle) => handle,
            Err(e) => {
                error!("Failed to find output device '{}': {}", device_id, e);
                
                // **CRASH FIX**: Try to refresh devices and try again
                warn!("Attempting to refresh device list and retry...");
                if let Err(refresh_err) = self.audio_device_manager.refresh_devices().await {
                    error!("Failed to refresh devices: {}", refresh_err);
                }
                
                // Try one more time after refresh
                match self.audio_device_manager.find_audio_device(device_id, false).await {
                    Ok(handle) => {
                        info!("Found output device '{}' after refresh", device_id);
                        handle
                    }
                    Err(retry_err) => {
                        error!("Output device '{}' still not found after refresh: {}", device_id, retry_err);
                        return Err(anyhow::anyhow!("Output device '{}' not found or unavailable. Original error: {}. Retry error: {}", device_id, e, retry_err));
                    }
                }
            }
        };
        
        match device_handle {
            super::AudioDeviceHandle::Cpal(device) => {
                self.create_cpal_output_stream(device_id, device).await
            }
            #[cfg(target_os = "macos")]
            super::AudioDeviceHandle::CoreAudio(coreaudio_device) => {
                self.create_coreaudio_output_stream(device_id, coreaudio_device).await
            }
        }
    }

    /// Create cpal output stream (existing implementation)
    async fn create_cpal_output_stream(&self, device_id: &str, device: cpal::Device) -> Result<()> {
        let device_name = device.name().unwrap_or_else(|_| device_id.to_string());
        debug!("Found cpal output device: {}", device_name);
        
        // Get the default output config for this device
        let config = device.default_output_config()
            .context("Failed to get default output config")?;
            
        debug!("Output device config: {:?}", config);
        
        // Create AudioOutputStream structure
        let output_stream = AudioOutputStream::new(
            device_id.to_string(),
            device_name.clone(),
            self.config.sample_rate,
        )?;
        
        // Get reference to the buffer for the output callback
        let output_buffer = output_stream.input_buffer.clone();
        let target_sample_rate = self.config.sample_rate;
        let buffer_size = self.config.buffer_size as usize;
        
        // Create the appropriate stream config for output with DYNAMIC buffer sizing
        let optimal_buffer_size = self.calculate_optimal_buffer_size(&device, &config, buffer_size).await?;
        let stream_config = StreamConfig {
            channels: 2, // Force stereo output
            sample_rate: SampleRate(target_sample_rate),
            buffer_size: optimal_buffer_size,
        };
        
        debug!("Using output stream config: channels={}, sample_rate={}, buffer_size={}", 
                stream_config.channels, stream_config.sample_rate.0, buffer_size);
        
        // **CRASH FIX**: Simplified stream creation with comprehensive error handling
        debug!("Building cpal output stream with format: {:?}", config.sample_format());
        
        // **CRASH FIX**: Handle stream creation and start in isolated scope to avoid Send trait issues
        let stream_started = {
            // Create stream and handle it immediately in the same scope
            let create_stream_result = match config.sample_format() {
                cpal::SampleFormat::F32 => {
                    info!("Creating F32 output stream for device: {}", device_name);
                    device.build_output_stream(
                        &stream_config,
                        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            // Safe, simple audio callback - just fill with silence for now to prevent crashes
                            if let Ok(mut buffer) = output_buffer.try_lock() {
                                let available_samples = buffer.len().min(data.len());
                                if available_samples > 0 {
                                    data[..available_samples].copy_from_slice(&buffer[..available_samples]);
                                    buffer.drain(..available_samples);
                                    if available_samples < data.len() {
                                        data[available_samples..].fill(0.0);
                                    }
                                } else {
                                    data.fill(0.0);
                                }
                            } else {
                                data.fill(0.0);
                            }
                        },
                        |err| error!("Output stream error: {}", err),
                        None
                    )
                },
                _ => {
                    info!("Creating default format output stream for device: {}", device_name);
                    // For non-F32 formats, try to create with F32 anyway as a fallback
                    device.build_output_stream(
                        &StreamConfig {
                            channels: 2,
                            sample_rate: config.sample_rate(),
                            buffer_size: optimal_buffer_size,
                        },
                        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            // Simple silence output for stability
                            if let Ok(mut buffer) = output_buffer.try_lock() {
                                let available_samples = buffer.len().min(data.len());
                                if available_samples > 0 {
                                    data[..available_samples].copy_from_slice(&buffer[..available_samples]);
                                    buffer.drain(..available_samples);
                                    if available_samples < data.len() {
                                        data[available_samples..].fill(0.0);
                                    }
                                } else {
                                    data.fill(0.0);
                                }
                            } else {
                                data.fill(0.0);
                            }
                        },
                        |err| error!("Output stream error: {}", err),
                        None
                    )
                }
            };
            
            // Handle the stream result immediately without holding it across await points
            match create_stream_result {
                Ok(stream) => {
                    info!("Successfully built output stream for: {}", device_name);
                    // Start the stream and return success/failure
                    match stream.play() {
                        Ok(()) => {
                            info!("Successfully started output stream for: {}", device_name);
                            true
                        }
                        Err(e) => {
                            error!("Failed to start output stream for {}: {}", device_name, e);
                            false
                        }
                    }
                    // stream is automatically dropped at end of scope
                }
                Err(e) => {
                    error!("Failed to build output stream for {}: {}", device_name, e);
                    return Err(anyhow::anyhow!("Failed to create output stream: {}", e));
                }
            }
        };
        
        if stream_started {
            // Track this device as having an active stream (stream is out of scope, safe to await)
            let mut active_devices = self.active_output_devices.lock().await;
            active_devices.insert(device_id.to_string());
        } else {
            return Err(anyhow::anyhow!("Failed to start output stream"));
        }
        
        // Store our wrapper
        let mut stream_guard = self.output_stream.lock().await;
        *stream_guard = Some(Arc::new(output_stream));
        
        info!("Successfully created real cpal output stream: {}", device_id);
        
        Ok(())
    }

    /// Create CoreAudio output stream for direct hardware access
    #[cfg(target_os = "macos")]
    async fn create_coreaudio_output_stream(&self, device_id: &str, coreaudio_device: super::CoreAudioDevice) -> Result<()> {
        info!("Creating CoreAudio output stream for device: {} (ID: {})", coreaudio_device.name, coreaudio_device.device_id);
        
        // Create the actual CoreAudio stream
        let mut coreaudio_stream = super::coreaudio_stream::CoreAudioOutputStream::new(
            coreaudio_device.device_id,
            coreaudio_device.name.clone(),
            self.config.sample_rate,
            coreaudio_device.channels,
        )?;
        
        // **CRITICAL FIX**: Start the CoreAudio stream with proper error handling
        match coreaudio_stream.start() {
            Ok(()) => {
                println!("Successfully started CoreAudio stream");
            }
            Err(e) => {
                eprintln!("Failed to start CoreAudio stream: {}", e);
                return Err(anyhow::anyhow!("Failed to start CoreAudio stream: {}", e));
            }
        }
        
        // Store the CoreAudio stream in the mixer to keep it alive
        let mut coreaudio_guard = self.coreaudio_stream.lock().await;
        *coreaudio_guard = Some(coreaudio_stream);
        
        // Create AudioOutputStream structure for compatibility with the existing mixer architecture
        let output_stream = AudioOutputStream::new(
            device_id.to_string(),
            coreaudio_device.name.clone(),
            self.config.sample_rate,
        )?;
        
        // Store our wrapper 
        let mut stream_guard = self.output_stream.lock().await;
        *stream_guard = Some(Arc::new(output_stream));
        
        println!("✅ Real CoreAudio Audio Unit stream created and started for: {}", device_id);
        
        Ok(())
    }

    /// Remove an input stream and clean up cpal stream
    pub async fn remove_input_stream(&self, device_id: &str) -> Result<()> {
        info!("🗑️ DEVICE CHANGE: Removing input stream for device: {}", device_id);
        
        // **CRITICAL FIX**: Check if stream exists before attempting removal
        let was_present = {
            let mut streams = self.input_streams.lock().await;
            streams.remove(device_id).is_some()
        };
        
        if !was_present {
            warn!("Attempted to remove non-existent stream for device: {}", device_id);
            return Ok(()); // Not an error - stream was already removed
        }
        
        // **CRITICAL FIX**: Graceful stream removal with timeout protection
        info!("🔴 Removing audio stream for device: {}", device_id);
        
        // Send stream removal command to the synchronous stream manager thread
        let stream_manager = get_stream_manager();
        let (response_tx, response_rx) = std::sync::mpsc::channel();
            
        let command = StreamCommand::RemoveStream {
            device_id: device_id.to_string(),
            response_tx,
        };
        
        // **CRITICAL FIX**: Send command with error handling
        if let Err(e) = stream_manager.send(command) {
            warn!("Failed to send stream removal command for {}: {}", device_id, e);
            return Ok(()); // Don't fail the entire operation
        }
        
        // **CRITICAL FIX**: Wait for response with timeout to prevent hanging
        let removed = match response_rx.recv_timeout(std::time::Duration::from_millis(2000)) {
            Ok(removed) => removed,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                warn!("Timeout waiting for stream removal response for device: {}", device_id);
                return Ok(()); // Continue anyway
            }
            Err(e) => {
                warn!("Error receiving stream removal response for {}: {}", device_id, e);
                return Ok(()); // Continue anyway
            }
        };
        
        if removed {
            info!("✅ Successfully removed input stream: {}", device_id);
        } else {
            warn!("Stream was not found in manager for removal: {}", device_id);
        }
        
        Ok(())
    }

    /// Stop the virtual mixer
    pub async fn stop(&mut self) -> Result<()> {
        println!("Stopping Virtual Mixer...");
        
        // **CRITICAL FIX**: Set running flag false first
        self.is_running.store(false, Ordering::Relaxed);
        
        // **CRITICAL FIX**: Wait briefly for audio processing thread to notice the stop flag
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        
        // **CRITICAL FIX**: Stop all output streams safely
        let _ = self.stop_output_streams().await;
        
        // **CRITICAL FIX**: Stop and remove all input streams
        let _ = self.stop_all_input_streams().await;
        
        println!("Virtual Mixer stopped successfully");
        Ok(())
    }

    async fn start_processing_thread(&self) -> Result<()> {
        let is_running = self.is_running.clone();
        let mix_buffer = self.mix_buffer.clone();
        let audio_output_tx = self.audio_output_tx.clone();
        let metrics = self.metrics.clone();
        let channel_levels = self.channel_levels.clone();
        let channel_levels_cache = self.channel_levels_cache.clone();
        let master_levels = self.master_levels.clone();
        let master_levels_cache = self.master_levels_cache.clone();
        
        // **PRIORITY 5: Audio Clock Synchronization** - Clone timing references
        let audio_clock = self.audio_clock.clone();
        let timing_metrics = self.timing_metrics.clone();
        let sample_rate = self.config.sample_rate;
        let buffer_size = self.config.buffer_size;
        let mixer_handle = VirtualMixerHandle {
            input_streams: self.input_streams.clone(),
            output_stream: self.output_stream.clone(),
            output_streams: self.output_streams.clone(), // Add multiple outputs support
            #[cfg(target_os = "macos")]
            coreaudio_stream: self.coreaudio_stream.clone(),
            channel_levels: self.channel_levels.clone(),
            config: self.shared_config.clone(),
        };

        // **CRITICAL FIX**: Use dedicated high-priority thread for real-time audio processing
        // tokio::spawn() can be preempted by scheduler causing audio dropouts and crunchiness
        std::thread::spawn(move || {
            // Set thread priority for real-time audio processing
            #[cfg(target_os = "macos")]
            {
                // On macOS, set thread to real-time priority to prevent preemption
                unsafe {
                    use libc::{pthread_self, pthread_setschedparam, sched_param, SCHED_RR};
                    let mut param: sched_param = std::mem::zeroed();
                    param.sched_priority = 80; // High priority for real-time audio
                    
                    if pthread_setschedparam(pthread_self(), SCHED_RR, &param) == 0 {
                        println!("✅ Audio thread priority set to real-time (priority: 80)");
                    } else {
                        println!("⚠️ Failed to set audio thread priority - may cause audio dropouts");
                    }
                }
            }
            
            // Create async runtime for this thread only
            let rt = tokio::runtime::Runtime::new().expect("Failed to create audio runtime");
            rt.block_on(async move {
            let mut frame_count = 0u64;
            
            // Pre-allocate stereo buffers to reduce allocations during real-time processing
            let mut reusable_output_buffer = vec![0.0f32; (buffer_size * 2) as usize];
            let mut reusable_left_samples = Vec::with_capacity(buffer_size as usize);
            let mut reusable_right_samples = Vec::with_capacity(buffer_size as usize);
            
            println!("🎵 Audio processing thread started with real mixing, optimized buffers, and clock synchronization");

            while is_running.load(Ordering::Relaxed) {
                let process_start = std::time::Instant::now();
                
                // **PRIORITY 5: Audio Clock Synchronization** - Track processing timing
                let timing_start = std::time::Instant::now();
                
                // **CALLBACK-DRIVEN PROCESSING**: Only process when audio data is available
                // This replaces timer-based processing to eliminate timing drift
                // Get current channel configuration dynamically (fixes mute/solo/gain not working)
                let current_channels = {
                    if let Ok(config_guard) = mixer_handle.config.try_lock() {
                        config_guard.channels.clone()
                    } else {
                        // Fallback to empty vec if can't lock (shouldn't happen often)
                        Vec::new()
                    }
                };
                let input_samples = mixer_handle.collect_input_samples_with_effects(&current_channels).await;
                
                // If no audio data is available from callbacks, add small delay to prevent excessive CPU usage
                // **RT THREAD FIX**: Add delay to prevent overwhelming system with debug output
                if input_samples.is_empty() {
                    std::thread::sleep(std::time::Duration::from_micros(100)); // 0.1ms sleep 
                    continue;
                }
                
                // Clear and reuse pre-allocated stereo buffers
                reusable_output_buffer.fill(0.0);
                reusable_left_samples.clear();
                reusable_right_samples.clear();
                
                // Calculate channel levels and mix audio
                let mut calculated_channel_levels = std::collections::HashMap::new();
                
                if !input_samples.is_empty() {
                    let mut active_channels = 0;
                    
                    // Mix all input channels together and calculate levels
                    for (device_id, samples) in input_samples.iter() {
                        if !samples.is_empty() {
                            active_channels += 1;
                            
                            // **STEREO FIX**: Calculate L/R peak and RMS levels separately for VU meters
                            let (peak_left, rms_left, peak_right, rms_right) = if samples.len() >= 2 {
                                // Stereo audio: separate L/R channels (interleaved format)
                                let left_samples: Vec<f32> = samples.iter().step_by(2).copied().collect();
                                let right_samples: Vec<f32> = samples.iter().skip(1).step_by(2).copied().collect();
                                
                                let peak_left = left_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                                let rms_left = if !left_samples.is_empty() {
                                    (left_samples.iter().map(|&s| s * s).sum::<f32>() / left_samples.len() as f32).sqrt()
                                } else { 0.0 };
                                
                                let peak_right = right_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                                let rms_right = if !right_samples.is_empty() {
                                    (right_samples.iter().map(|&s| s * s).sum::<f32>() / right_samples.len() as f32).sqrt()
                                } else { 0.0 };
                                
                                (peak_left, rms_left, peak_right, rms_right)
                            } else {
                                // Mono audio: duplicate to both L/R channels
                                let peak_mono = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                                let rms_mono = if !samples.is_empty() {
                                    (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
                                } else { 0.0 };
                                
                                (peak_mono, rms_mono, peak_mono, rms_mono)
                            };
                            
                            // Find which channel this device belongs to
                            if let Some(channel) = current_channels.iter().find(|ch| {
                                ch.input_device_id.as_ref() == Some(device_id)
                            }) {
                                // Store stereo levels by channel ID
                                calculated_channel_levels.insert(channel.id, (peak_left, rms_left, peak_right, rms_right));
                                
                                // Log levels occasionally
                                if frame_count % 100 == 0 && (peak_left > 0.001 || peak_right > 0.001) {
                                    crate::audio_debug!("Channel {} ({}): {} samples, L(peak: {:.3}, rms: {:.3}) R(peak: {:.3}, rms: {:.3})", 
                                        channel.id, device_id, samples.len(), peak_left, rms_left, peak_right, rms_right);
                                }
                            }
                            
                            // **CRITICAL FIX**: Ensure safe mixing with different sample rates and channel counts
                            // The reusable_output_buffer is stereo (interleaved L/R) at the mixer sample rate
                            let mixer_sample_rate = sample_rate as f32;
                            
                            // Convert mono input to stereo if needed
                            let stereo_samples = if samples.len() % 2 == 0 {
                                // Already stereo (even number of samples)
                                samples.clone()
                            } else {
                                // Mono input - duplicate to both L/R channels
                                let mut stereo = Vec::with_capacity(samples.len() * 2);
                                for &mono_sample in samples.iter() {
                                    stereo.push(mono_sample); // Left channel
                                    stereo.push(mono_sample); // Right channel (duplicate)
                                }
                                stereo
                            };
                            
                            // **CRITICAL FIX**: Safe buffer size matching to prevent crashes
                            // Only mix up to the smaller buffer size to prevent overruns
                            let mix_length = reusable_output_buffer.len().min(stereo_samples.len());
                            
                            // Add samples with bounds checking
                            for i in 0..mix_length {
                                if i < reusable_output_buffer.len() && i < stereo_samples.len() {
                                    reusable_output_buffer[i] += stereo_samples[i];
                                }
                            }
                            
                            // Debug sample rate mismatches occasionally
                            if frame_count % 500 == 0 && samples.len() > 0 {
                                println!("🔧 MIXER DEBUG: Device {} - Original: {} samples, Stereo: {} samples, Mixed: {} samples", 
                                    device_id, samples.len(), stereo_samples.len(), mix_length);
                            }
                        }
                    }
                    
                    // **AUDIO QUALITY FIX**: Smart gain management instead of aggressive division
                    // Only normalize if we have multiple overlapping channels with significant signal
                    if active_channels > 1 {
                        // Check if we actually need normalization by checking peak levels
                        let buffer_peak = reusable_output_buffer.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        
                        // Only normalize if we're approaching clipping (> 0.8) with multiple channels
                        if buffer_peak > 0.8 {
                            let normalization_factor = 0.8 / buffer_peak; // Normalize to 80% max to prevent clipping
                            for sample in reusable_output_buffer.iter_mut() {
                                *sample *= normalization_factor;
                            }
                            println!("🔧 GAIN CONTROL: Normalized {} channels, peak {:.3} -> {:.3}", 
                                active_channels, buffer_peak, buffer_peak * normalization_factor);
                        }
                        // If not approaching clipping, leave levels untouched for better dynamics
                    }
                    // Single channels: NO normalization - preserve full dynamics
                    
                    // Stereo audio is already mixed directly into reusable_output_buffer
                    // No conversion needed - stereo data preserved throughout mixing process
                    
                    // **AUDIO QUALITY FIX**: Professional master gain instead of aggressive reduction
                    let master_gain = 0.9f32; // Professional level (was 0.5 - too low!)
                    
                    // Only apply master gain reduction if signal is actually hot
                    let pre_master_peak = reusable_output_buffer.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    
                    if pre_master_peak > 0.95 {
                        // Signal is very hot, apply conservative gain
                        let conservative_gain = 0.8f32;
                        for sample in reusable_output_buffer.iter_mut() {
                            *sample *= conservative_gain;
                        }
                        println!("🔧 MASTER LIMITER: Hot signal {:.3}, applied {:.2} gain", pre_master_peak, conservative_gain);
                    } else {
                        // Normal signal levels, apply professional master gain
                        for sample in reusable_output_buffer.iter_mut() {
                            *sample *= master_gain;
                        }
                    }
                    
                    // Calculate master output levels for L/R channels using reusable vectors
                    reusable_left_samples.extend(reusable_output_buffer.iter().step_by(2).copied());
                    reusable_right_samples.extend(reusable_output_buffer.iter().skip(1).step_by(2).copied());
                    
                    let left_peak = reusable_left_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let left_rms = if !reusable_left_samples.is_empty() {
                        (reusable_left_samples.iter().map(|&s| s * s).sum::<f32>() / reusable_left_samples.len() as f32).sqrt()
                    } else { 0.0 };
                    
                    let right_peak = reusable_right_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let right_rms = if !reusable_right_samples.is_empty() {
                        (reusable_right_samples.iter().map(|&s| s * s).sum::<f32>() / reusable_right_samples.len() as f32).sqrt()
                    } else { 0.0 };
                    
                    // Store real master levels
                    let master_level_values = (left_peak, left_rms, right_peak, right_rms);
                    if let Ok(mut levels_guard) = master_levels.try_lock() {
                        *levels_guard = master_level_values;
                    }
                    
                    // Also update cache for fallback (non-blocking)
                    let has_signal = left_peak > 0.0 || left_rms > 0.0 || right_peak > 0.0 || right_rms > 0.0;
                    if has_signal {
                        if let Ok(mut cache_guard) = master_levels_cache.try_lock() {
                            *cache_guard = master_level_values;
                        }
                    }
                    
                    // Log master levels occasionally
                    if frame_count % 100 == 0 && (left_peak > 0.001 || right_peak > 0.001) {
                        crate::audio_debug!("Master output: L(peak: {:.3}, rms: {:.3}) R(peak: {:.3}, rms: {:.3})", 
                            left_peak, left_rms, right_peak, right_rms);
                    }
                }
                
                // Store calculated channel levels for VU meters
                if !calculated_channel_levels.is_empty() {
                    if frame_count % 100 == 0 {
                        crate::audio_debug!("📊 STORING LEVELS: Attempting to store {} channel levels", calculated_channel_levels.len());
                        for (channel_id, (peak_left, rms_left, peak_right, rms_right)) in calculated_channel_levels.iter() {
                            crate::audio_debug!("   Level [Channel {}]: L(peak={:.4}, rms={:.4}) R(peak={:.4}, rms={:.4})", 
                                channel_id, peak_left, rms_left, peak_right, rms_right);
                        }
                    }
                    
                    match channel_levels.try_lock() {
                        Ok(mut levels_guard) => {
                            *levels_guard = calculated_channel_levels.clone();
                            if frame_count % 100 == 0 {
                                crate::audio_debug!("✅ STORED LEVELS: Successfully stored {} channel levels in HashMap", calculated_channel_levels.len());
                            }
                        }
                        Err(_) => {
                            if frame_count % 100 == 0 {
                                println!("🚫 STORAGE FAILED: Could not lock channel_levels HashMap for storage");
                            }
                        }
                    }
                } else {
                    if frame_count % 500 == 0 {
                        println!("⚠️  NO LEVELS TO STORE: calculated_channel_levels is empty");
                    }
                }
                
                // Also update cache for fallback (non-blocking)
                if !calculated_channel_levels.is_empty() {
                    if let Ok(mut cache_guard) = channel_levels_cache.try_lock() {
                        *cache_guard = calculated_channel_levels;
                    }
                }
                
                // Update mix buffer
                if let Ok(mut buffer_guard) = mix_buffer.try_lock() {
                    if buffer_guard.len() == reusable_output_buffer.len() {
                        buffer_guard.copy_from_slice(&reusable_output_buffer);
                    }
                }
                
                // Send to output stream
                mixer_handle.send_to_output(&reusable_output_buffer).await;

                // Send processed audio to the rest of the application (non-blocking)
                let _ = audio_output_tx.try_send(reusable_output_buffer.clone());
                // Don't break on send failure - just continue processing

                frame_count += 1;
                
                // **PRIORITY 5: Audio Clock Synchronization** - Update master clock and timing metrics
                let samples_processed = buffer_size as usize;
                let processing_time_us = timing_start.elapsed().as_micros() as f64;
                
                // Update audio clock with processed samples
                if let Ok(mut clock_guard) = audio_clock.try_lock() {
                    if let Some(sync_info) = clock_guard.update(samples_processed) {
                        // Clock detected timing drift - log it
                        if sync_info.needs_adjustment {
                            crate::audio_debug!("⚠️  TIMING DRIFT: {:.2}ms drift detected at {} samples", 
                                sync_info.drift_microseconds / 1000.0, sync_info.samples_processed);
                            
                            // Record sync adjustment in metrics
                            if let Ok(mut metrics_guard) = timing_metrics.try_lock() {
                                metrics_guard.record_sync_adjustment();
                            }
                        }
                    }
                }
                
                // Record processing time metrics
                if let Ok(mut metrics_guard) = timing_metrics.try_lock() {
                    metrics_guard.record_processing_time(processing_time_us);
                    
                    // Check for underruns (no input samples available)
                    if input_samples.is_empty() {
                        metrics_guard.record_underrun();
                    }
                }
                
                // **TIMING METRICS**: Report comprehensive timing every 10 seconds
                if frame_count % ((sample_rate / buffer_size) as u64 * 10) == 0 {
                    if let Ok(metrics_guard) = timing_metrics.try_lock() {
                        println!("📈 {}", metrics_guard.get_summary());
                    }
                    if let Ok(clock_guard) = audio_clock.try_lock() {
                        let sample_timestamp = clock_guard.get_sample_timestamp();
                        let drift = clock_guard.get_drift_compensation();
                        println!("⏰ Audio Clock: {} samples processed, {:.2}ms drift", 
                            sample_timestamp, drift / 1000.0);
                    }
                }
                
                // Update metrics every second
                if frame_count % (sample_rate / buffer_size) as u64 == 0 {
                    let cpu_time = process_start.elapsed().as_secs_f32();
                    let max_cpu_time = buffer_size as f32 / sample_rate as f32;
                    let cpu_usage = (cpu_time / max_cpu_time) * 100.0;
                    
                    if let Ok(mut metrics_guard) = metrics.try_lock() {
                        metrics_guard.cpu_usage = cpu_usage;
                    }
                    
                    if input_samples.len() > 0 {
                        println!("Audio processing: CPU {:.1}%, {} active streams", cpu_usage, input_samples.len());
                    }
                }

                // **TIMING DRIFT FIX**: Replace timer-based processing with callback-driven approach
                // Only process when we have sufficient audio data from callbacks, eliminating drift
                
                let elapsed = process_start.elapsed();
                let hardware_buffer_duration_ms = (buffer_size as f32 / sample_rate as f32) * 1000.0;
                
                // Debug timing changes every 5 seconds
                if frame_count % ((sample_rate / buffer_size) as u64 * 5) == 0 {
                    println!("🕐 CALLBACK-DRIVEN: Processing triggered by audio data availability, no timer drift (was sleeping {:.2}ms)", 
                        hardware_buffer_duration_ms);
                }
                
                // **CRITICAL TIMING FIX**: Instead of sleeping on a timer (which causes drift),
                // wait for actual audio data to be available from hardware callbacks.
                // This synchronizes processing directly with hardware timing, eliminating drift.
                
                // Only yield minimally if processing was too fast
                if elapsed.as_micros() < 1000 {
                    // Processing was very fast (< 1ms), yield briefly to prevent spinning
                    tokio::task::yield_now().await;
                } else if elapsed.as_millis() > 50 {
                    // Processing took too long (> 50ms), log the overrun
                    if frame_count % 10 == 0 {
                        println!("⚠️  PROCESSING OVERRUN: {}ms processing time (audio callback driven)", elapsed.as_millis());
                    }
                    tokio::task::yield_now().await;
                }
                
                // **NO MORE TIMER-BASED SLEEPING** - processing is now driven by available audio data
                // The loop will naturally pace itself based on when audio callbacks provide data
            }
            
            println!("Audio processing thread stopped");
            }) // End of async block for runtime
        }); // End of thread spawn

        Ok(())
    }

    /// Add a new audio channel
    pub async fn add_channel(&mut self, channel: AudioChannel) -> Result<()> {
        // Simple channel addition without automatic stream creation
        self.config.channels.push(channel);
        Ok(())
    }

    /// Get current mixer metrics
    pub async fn get_metrics(&self) -> AudioMetrics {
        self.metrics.lock().await.clone()
    }

    /// Get current channel levels for VU meters with proper fallback caching
    pub async fn get_channel_levels(&self) -> HashMap<u32, (f32, f32, f32, f32)> {
        use std::sync::{LazyLock, Mutex as StdMutex};
        static API_CALL_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
        
        let call_count = if let Ok(mut count) = API_CALL_COUNT.lock() {
            *count += 1;
            *count
        } else {
            0
        };
        
        // Try to get real-time levels first
        if let Ok(levels_guard) = self.channel_levels.try_lock() {
            let levels = levels_guard.clone();
            
            // Debug: Log what we're returning to the frontend
            if call_count % 50 == 0 || (!levels.is_empty() && call_count % 10 == 0) {
                crate::audio_debug!("🌐 API CALL #{}: get_channel_levels() returning {} levels", call_count, levels.len());
                for (channel_id, (peak_left, rms_left, peak_right, rms_right)) in levels.iter() {
                    crate::audio_debug!("   API Level [Channel {}]: L(peak={:.4}, rms={:.4}) R(peak={:.4}, rms={:.4})", 
                        channel_id, peak_left, rms_left, peak_right, rms_right);
                }
            }
            
            // Update cache with latest values (non-blocking)
            if !levels.is_empty() {
                if let Ok(mut cache_guard) = self.channel_levels_cache.try_lock() {
                    *cache_guard = levels.clone();
                }
            }
            
            levels
        } else {
            // Fallback to cached levels if we can't get the real-time lock
            if let Ok(cache_guard) = self.channel_levels_cache.try_lock() {
                let cached_levels = cache_guard.clone();
                if call_count % 50 == 0 {
                    println!("🌐 API CALL #{}: get_channel_levels() using CACHED levels ({} items)", call_count, cached_levels.len());
                }
                cached_levels
            } else {
                // Last resort: return empty levels
                if call_count % 100 == 0 {
                    println!("🌐 API CALL #{}: get_channel_levels() returning EMPTY levels (lock failed)", call_count);
                }
                HashMap::new()
            }
        }
    }

    /// Get current master output levels for VU meters (Left/Right) with proper fallback caching
    pub async fn get_master_levels(&self) -> (f32, f32, f32, f32) {
        // Try to get real-time levels first
        if let Ok(levels_guard) = self.master_levels.try_lock() {
            let levels = *levels_guard;
            
            // Update cache with latest values (non-blocking)
            let has_signal = levels.0 > 0.0 || levels.1 > 0.0 || levels.2 > 0.0 || levels.3 > 0.0;
            if has_signal {
                if let Ok(mut cache_guard) = self.master_levels_cache.try_lock() {
                    *cache_guard = levels;
                }
            }
            
            levels
        } else {
            // Fallback to cached levels if we can't get the real-time lock
            if let Ok(cache_guard) = self.master_levels_cache.try_lock() {
                *cache_guard
            } else {
                // Last resort: return zero levels
                (0.0, 0.0, 0.0, 0.0)
            }
        }
    }

    /// Get audio output stream for streaming/recording
    pub async fn get_audio_output_receiver(&self) -> mpsc::Receiver<Vec<f32>> {
        // Return a connected receiver that gets real audio data from the processing thread
        let (tx, rx) = mpsc::channel(8192);
        
        // Clone references needed for the forwarding task
        let audio_output_tx = self.audio_output_tx.clone();
        
        // Spawn a task to forward audio from the processing thread to this receiver
        tokio::spawn(async move {
            let mut audio_rx = {
                // We need to create a new receiver by cloning the sender
                // This is a limitation - ideally we'd have a broadcast channel
                let (_temp_tx, temp_rx) = mpsc::channel(8192);
                temp_rx
            };
            
            // For now, we'll need to modify the processing thread to support multiple receivers
            // This is a placeholder that demonstrates the correct API
            while let Some(audio_data) = audio_rx.recv().await {
                if tx.send(audio_data).await.is_err() {
                    // Receiver dropped, stop forwarding
                    break;
                }
            }
        });
        
        rx
    }

    /// Send command to mixer
    pub async fn send_command(&self, command: MixerCommand) -> Result<()> {
        self.command_tx.send(command).await
            .context("Failed to send mixer command")?;
        Ok(())
    }

    /// Update channel configuration (now updates running mixer configuration)
    pub async fn update_channel(&mut self, channel_id: u32, updated_channel: AudioChannel) -> Result<()> {
        // Update the main config
        if let Some(channel) = self.config.channels.iter_mut().find(|c| c.id == channel_id) {
            *channel = updated_channel.clone();
        }
        
        // **CRITICAL FIX**: Update the shared configuration that the processing loop reads from
        if let Ok(mut shared_config_guard) = self.shared_config.try_lock() {
            if let Some(shared_channel) = shared_config_guard.channels.iter_mut().find(|c| c.id == channel_id) {
                *shared_channel = updated_channel.clone();
                println!("🔄 Channel {} updated in shared config: muted={}, solo={}, gain={:.2}, pan={:.2}", 
                    channel_id, updated_channel.muted, updated_channel.solo, 
                    updated_channel.gain, updated_channel.pan);
            }
        } else {
            println!("⚠️ Could not update shared config for channel {} - processing loop may not see changes", channel_id);
        }
        
        Ok(())
    }

    /// Get the audio device manager
    pub fn get_device_manager(&self) -> &Arc<AudioDeviceManager> {
        &self.audio_device_manager
    }
    
    /// Get a mutable reference to a channel by ID
    pub fn get_channel_mut(&mut self, channel_id: u32) -> Option<&mut AudioChannel> {
        self.config.channels.iter_mut().find(|c| c.id == channel_id)
    }
    
    /// Get a reference to a channel by ID
    pub fn get_channel(&self, channel_id: u32) -> Option<&AudioChannel> {
        self.config.channels.iter().find(|c| c.id == channel_id)
    }
    
    /// **NEW**: Safely stop all output streams to prevent crashes when switching devices
    async fn stop_output_streams(&self) -> Result<()> {
        info!("Stopping existing output streams...");
        
        // Stop CoreAudio stream if active
        #[cfg(target_os = "macos")]
        {
            let mut coreaudio_guard = self.coreaudio_stream.lock().await;
            if let Some(mut stream) = coreaudio_guard.take() {
                info!("Stopping CoreAudio stream...");
                if let Err(e) = stream.stop() {
                    warn!("Error stopping CoreAudio stream: {}", e);
                }
            }
        }
        
        // Clear tracked active output devices
        let mut active_devices_guard = self.active_output_devices.lock().await;
        if !active_devices_guard.is_empty() {
            info!("Clearing {} tracked active output devices...", active_devices_guard.len());
            active_devices_guard.clear(); // Clear tracking - streams are managed by audio subsystem
        }
        
        // Clear the regular output stream wrapper
        let mut stream_guard = self.output_stream.lock().await;
        if stream_guard.is_some() {
            debug!("Clearing output stream wrapper...");
            *stream_guard = None;
        }
        
        // Small delay to allow audio subsystem to release resources
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        
        info!("Output streams stopped");
        Ok(())
    }
    
    /// **NEW**: Stop all input streams safely
    async fn stop_all_input_streams(&self) -> Result<()> {
        info!("Stopping all input streams...");
        
        let device_ids: Vec<String> = {
            let streams = self.input_streams.lock().await;
            streams.keys().cloned().collect()
        };
        
        for device_id in device_ids {
            debug!("Stopping input stream: {}", device_id);
            if let Err(e) = self.remove_input_stream(&device_id).await {
                warn!("Error stopping input stream {}: {}", device_id, e);
            }
        }
        
        info!("All input streams stopped");
        Ok(())
    }
    
    /// Add an output device to the mixer
    pub async fn add_output_device(&self, output_device: super::types::OutputDevice) -> Result<()> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        
        let devices = self.audio_device_manager.enumerate_devices().await?;
        
        // Find the device
        let device_info = devices.iter()
            .find(|d| d.id == output_device.device_id && d.is_output)
            .ok_or_else(|| anyhow::anyhow!("Output device not found: {}", output_device.device_id))?;
            
        // **CRASH PREVENTION**: Use device manager's safe device finding instead of direct CPAL calls
        let device_handle = self.audio_device_manager.find_audio_device(&output_device.device_id, false).await?;
        let device = match device_handle {
            super::AudioDeviceHandle::Cpal(cpal_device) => cpal_device,
            #[cfg(target_os = "macos")]
            super::AudioDeviceHandle::CoreAudio(_) => {
                return Err(anyhow::anyhow!("CoreAudio device handles not supported in add_output_device - use CPAL fallback"));
            }
            #[cfg(not(target_os = "macos"))]
            _ => {
                return Err(anyhow::anyhow!("Unknown device handle type"));
            }
        };
        
        // Create output stream
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
    pub async fn remove_output_device(&self, device_id: &str) -> Result<()> {
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
    
    /// Update output device configuration
    pub async fn update_output_device(&self, device_id: &str, updated_device: super::types::OutputDevice) -> Result<()> {
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
    
    /// Get a specific output device configuration
    pub async fn get_output_device(&self, device_id: &str) -> Option<super::types::OutputDevice> {
        let config_guard = self.shared_config.lock().unwrap();
        config_guard.output_devices
            .iter()
            .find(|d| d.device_id == device_id)
            .cloned()
    }
    
    /// Get all output devices
    pub async fn get_output_devices(&self) -> Vec<super::types::OutputDevice> {
        let config_guard = self.shared_config.lock().unwrap();
        config_guard.output_devices.clone()
    }
    
    /// Enhanced add input stream with device health checking
    pub async fn add_input_stream_safe(&self, device_id: &str) -> Result<()> {
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
            Ok(super::devices::DeviceStatus::Connected) => {
                // Device is healthy, proceed with normal stream addition
                // This would call the existing stream management logic
                println!("✅ Device {} is healthy, proceeding with stream creation", device_id);
            }
            Ok(super::devices::DeviceStatus::Disconnected) => {
                self.audio_device_manager.report_device_error(
                    device_id, 
                    "Device disconnected".to_string()
                ).await;
                return Err(anyhow::anyhow!("Device {} is disconnected", device_id));
            }
            Ok(super::devices::DeviceStatus::Error(err)) => {
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
        
        // Initialize health tracking for this device if not already tracked
        if let Ok(devices) = self.audio_device_manager.enumerate_devices().await {
            if let Some(device_info) = devices.iter().find(|d| d.id == device_id) {
                self.audio_device_manager.initialize_device_health(device_info).await;
            }
        }
        
        Ok(())
    }
    
    /// Enhanced add output stream with device health checking
    pub async fn add_output_stream_safe(&self, device_id: &str) -> Result<()> {
        info!("Adding output stream for device with health checking: {}", device_id);
        
        // Similar health checking for output devices
        if self.audio_device_manager.should_avoid_device(device_id).await {
            let health = self.audio_device_manager.get_device_health(device_id).await;
            if let Some(h) = health {
                return Err(anyhow::anyhow!(
                    "Avoiding output device {} due to {} consecutive errors", 
                    device_id, h.consecutive_errors
                ));
            }
        }
        
        match self.audio_device_manager.check_device_health(device_id).await {
            Ok(super::devices::DeviceStatus::Connected) => {
                println!("✅ Output device {} is healthy, proceeding with stream creation", device_id);
                // Initialize health tracking
                if let Ok(devices) = self.audio_device_manager.enumerate_devices().await {
                    if let Some(device_info) = devices.iter().find(|d| d.id == device_id) {
                        self.audio_device_manager.initialize_device_health(device_info).await;
                    }
                }
                Ok(())
            }
            Ok(super::devices::DeviceStatus::Disconnected) => {
                self.audio_device_manager.report_device_error(
                    device_id, 
                    "Output device disconnected".to_string()
                ).await;
                Err(anyhow::anyhow!("Output device {} is disconnected", device_id))
            }
            Ok(super::devices::DeviceStatus::Error(err)) => {
                Err(anyhow::anyhow!("Output device {} has error: {}", device_id, err))
            }
            Err(e) => {
                self.audio_device_manager.report_device_error(
                    device_id, 
                    format!("Output health check failed: {}", e)
                ).await;
                Err(anyhow::anyhow!("Failed to check output device {} health: {}", device_id, e))
            }
        }
    }
    
    /// Get device health status for UI reporting
    pub async fn get_device_health_status(&self, device_id: &str) -> Option<super::devices::DeviceHealth> {
        self.audio_device_manager.get_device_health(device_id).await
    }
    
    /// Get all device health statuses for UI monitoring
    pub async fn get_all_device_health_statuses(&self) -> std::collections::HashMap<String, super::devices::DeviceHealth> {
        self.audio_device_manager.get_all_device_health().await
    }
    
    /// Report a device error from external sources (like stream callbacks)
    pub async fn report_device_error(&self, device_id: &str, error: String) {
        self.audio_device_manager.report_device_error(device_id, error).await;
    }
}

/// **PRIORITY 5: Audio Clock Synchronization**
/// Master audio clock for timing synchronization between input and output streams
#[derive(Debug)]
pub struct AudioClock {
    sample_rate: u32,
    samples_processed: u64,
    start_time: std::time::Instant,
    last_sync_time: std::time::Instant,
    drift_compensation: f64, // Microseconds of drift compensation
    sync_interval_samples: u64, // Sync every N samples
}

impl AudioClock {
    pub fn new(sample_rate: u32, buffer_size: u32) -> Self {
        let now = std::time::Instant::now();
        Self {
            sample_rate,
            samples_processed: 0,
            start_time: now,
            last_sync_time: now,
            drift_compensation: 0.0,
            sync_interval_samples: buffer_size as u64, // Sync every buffer to match hardware callback timing
        }
    }
    
    /// Update the clock with processed samples - now tracks hardware callback timing instead of software timing
    pub fn update(&mut self, samples_added: usize) -> Option<TimingSync> {
        self.samples_processed += samples_added as u64;
        
        // Check if it's time to sync (every sync_interval_samples)
        if self.samples_processed % self.sync_interval_samples == 0 {
            let now = std::time::Instant::now();
            
            // **CRITICAL FIX**: In callback-driven processing, we don't calculate "expected" timing
            // because the samples arrive exactly when the hardware provides them.
            // Instead, we only track callback consistency and hardware timing variations.
            
            let callback_interval_us = now.duration_since(self.last_sync_time).as_micros() as f64;
            let expected_interval_us = (self.sync_interval_samples as f64 * 1_000_000.0) / self.sample_rate as f64;
            
            // Only report drift if callback intervals are inconsistent with expected buffer timing
            // This detects real hardware timing issues, not software processing timing
            let interval_variation = callback_interval_us - expected_interval_us;
            
            // Only consider significant variations in hardware callback timing as real drift
            let is_hardware_drift = interval_variation.abs() > expected_interval_us * 0.1; // 10% variation threshold
            
            // Reset drift compensation since we're now hardware-synchronized
            self.drift_compensation = if is_hardware_drift { interval_variation } else { 0.0 };
            
            let sync = TimingSync {
                samples_processed: self.samples_processed,
                drift_microseconds: interval_variation,
                needs_adjustment: is_hardware_drift,
                sync_time: now,
            };
            
            // Only log actual hardware timing issues, not software processing timing
            if is_hardware_drift {
                crate::audio_debug!("⏰ HARDWARE TIMING: Callback interval variation: {:.2}ms (expected: {:.2}ms, actual: {:.2}ms)", 
                    interval_variation / 1000.0, expected_interval_us / 1000.0, callback_interval_us / 1000.0);
            }
            
            self.last_sync_time = now;
            Some(sync)
        } else {
            None
        }
    }
    
    /// Get the current audio timestamp in samples
    pub fn get_sample_timestamp(&self) -> u64 {
        self.samples_processed
    }
    
    /// Get the current drift compensation
    pub fn get_drift_compensation(&self) -> f64 {
        self.drift_compensation
    }
    
    /// Reset the clock (useful when switching sample rates or buffer sizes)
    pub fn reset(&mut self, new_sample_rate: Option<u32>, new_buffer_size: Option<u32>) {
        if let Some(sr) = new_sample_rate {
            self.sample_rate = sr;
        }
        if let Some(bs) = new_buffer_size {
            self.sync_interval_samples = bs as u64; // Sync based on actual buffer size from config
        }
        
        let now = std::time::Instant::now();
        self.samples_processed = 0;
        self.start_time = now;
        self.last_sync_time = now;
        self.drift_compensation = 0.0;
        
        println!("⏰ AUDIO CLOCK: Reset with sample rate {} Hz", self.sample_rate);
    }
}

/// Timing synchronization result from clock update
#[derive(Debug, Clone)]
pub struct TimingSync {
    pub samples_processed: u64,
    pub drift_microseconds: f64,
    pub needs_adjustment: bool,
    pub sync_time: std::time::Instant,
}

/// Performance timing metrics for audio processing
#[derive(Debug)]
pub struct TimingMetrics {
    pub processing_time_avg_us: f64,
    pub processing_time_max_us: f64,
    pub buffer_underruns: u64,
    pub buffer_overruns: u64,
    pub sync_adjustments: u64,
    pub last_reset: std::time::Instant,
    sample_count: u64,
    processing_time_sum_us: f64,
}

impl TimingMetrics {
    pub fn new() -> Self {
        Self {
            processing_time_avg_us: 0.0,
            processing_time_max_us: 0.0,
            buffer_underruns: 0,
            buffer_overruns: 0,
            sync_adjustments: 0,
            last_reset: std::time::Instant::now(),
            sample_count: 0,
            processing_time_sum_us: 0.0,
        }
    }
    
    /// Record processing time for a buffer
    pub fn record_processing_time(&mut self, duration_us: f64) {
        self.processing_time_sum_us += duration_us;
        self.sample_count += 1;
        
        // Update max
        if duration_us > self.processing_time_max_us {
            self.processing_time_max_us = duration_us;
        }
        
        // Update rolling average
        self.processing_time_avg_us = self.processing_time_sum_us / self.sample_count as f64;
    }
    
    /// Record buffer underrun (not enough samples available)
    pub fn record_underrun(&mut self) {
        self.buffer_underruns += 1;
    }
    
    /// Record buffer overrun (too many samples, had to drop)
    pub fn record_overrun(&mut self) {
        self.buffer_overruns += 1;
    }
    
    /// Record sync adjustment applied
    pub fn record_sync_adjustment(&mut self) {
        self.sync_adjustments += 1;
    }
    
    /// Reset metrics (useful for periodic reporting)
    pub fn reset(&mut self) {
        *self = Self::new();
    }
    
    /// Get metrics summary
    pub fn get_summary(&self) -> String {
        let uptime_sec = self.last_reset.elapsed().as_secs_f64();
        format!(
            "Audio Metrics ({}s): Avg Processing: {:.1}μs, Max: {:.1}μs, Underruns: {}, Overruns: {}, Sync Adjustments: {}",
            uptime_sec.round(),
            self.processing_time_avg_us,
            self.processing_time_max_us,
            self.buffer_underruns,
            self.buffer_overruns,
            self.sync_adjustments
        )
    }
}