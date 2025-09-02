// Stream operations for VirtualMixer
//
// This module contains all the VirtualMixer methods related to stream lifecycle
// management, including adding/removing input/output streams, device switching,
// and stream configuration operations.

use anyhow::{Context, Result};
use std::sync::{atomic::Ordering, Arc};
use tracing::{info, warn, error};
use cpal::traits::DeviceTrait;

use super::types::VirtualMixer;
use super::stream_management::{AudioInputStream, AudioOutputStream, StreamCommand, StreamInfo, get_stream_manager};
use super::mixer_core::VirtualMixerHandle;

impl VirtualMixer {
    /// Start the mixer and initialize audio processing
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            warn!("Mixer is already running");
            return Ok(());
        }

        info!("🚀 MIXER START: Starting virtual mixer...");
        
        // Reset timing metrics
        {
            let mut timing_metrics = self.timing_metrics.lock().await;
            timing_metrics.reset();
        }
        
        // Reset audio clock
        {
            let mut audio_clock = self.audio_clock.lock().await;
            audio_clock.reset();
        }
        
        self.is_running.store(true, Ordering::Relaxed);
        
        // Start the audio processing thread (implementation moved from stream_management.rs)
        self.start_processing_thread().await?;
        
        info!("✅ MIXER STARTED: Virtual mixer started successfully");
        
        Ok(())
    }

    /// Stop the mixer and cleanup resources
    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            info!("Mixer is already stopped");
            return Ok(());
        }

        info!("🛑 MIXER STOP: Stopping virtual mixer...");
        
        self.is_running.store(false, Ordering::Relaxed);
        
        // Clear all input streams
        {
            let mut input_streams = self.input_streams.lock().await;
            input_streams.clear();
        }
        
        // Clear output streams
        {
            let mut output_stream = self.output_stream.lock().await;
            *output_stream = None;
        }
        
        {
            let mut output_streams = self.output_streams.lock().await;
            output_streams.clear();
        }
        
        // Clear active devices tracking
        {
            let mut active_devices = self.active_output_devices.lock().await;
            active_devices.clear();
        }
        
        #[cfg(target_os = "macos")]
        {
            let mut coreaudio_stream = self.coreaudio_stream.lock().await;
            *coreaudio_stream = None;
        }
        
        info!("✅ MIXER STOPPED: Virtual mixer stopped successfully");
        Ok(())
    }

    /// Get information about active streams
    pub async fn get_stream_info(&self) -> StreamInfo {
        let input_count = {
            let input_streams = self.input_streams.lock().await;
            input_streams.len()
        };
        
        let output_count = {
            let output_streams = self.output_streams.lock().await;
            output_streams.len()
        };
        
        let active_devices = {
            let active_devices = self.active_output_devices.lock().await;
            active_devices.clone()
        };
        
        let is_running = self.is_running.load(Ordering::Relaxed);
        
        StreamInfo {
            is_running,
            input_stream_count: input_count,
            output_stream_count: output_count,
            active_output_devices: active_devices.into_iter().collect(),
        }
    }

    /// Check if a specific device is currently active
    pub async fn is_device_active(&self, device_id: &str) -> bool {
        // Check input streams
        {
            let input_streams = self.input_streams.lock().await;
            if input_streams.contains_key(device_id) {
                return true;
            }
        }
        
        // Check output streams
        {
            let active_devices = self.active_output_devices.lock().await;
            if active_devices.contains(device_id) {
                return true;
            }
        }
        
        false
    }

    /// Calculate optimal buffer size for a specific device
    async fn calculate_optimal_buffer_size(&self, device: &cpal::Device, config: &cpal::SupportedStreamConfig, requested_size: usize) -> Result<cpal::BufferSize> {
        // For now, use default buffer size to ensure compatibility
        // Could be enhanced later with device-specific optimization
        Ok(cpal::BufferSize::Default)
    }

    /// Add an input stream for the specified device
    pub async fn add_input_stream(&self, device_id: &str) -> Result<()> {
        super::validation::validate_device_id(device_id)?;
        
        info!("🔌 INPUT STREAM: Adding input stream for device: {}", device_id);
        
        // Check if stream already exists
        {
            let input_streams = self.input_streams.lock().await;
            if input_streams.contains_key(device_id) {
                warn!("Input stream for device '{}' already exists", device_id);
                return Ok(());
            }
        }
        
        // **CRITICAL FIX**: Use find_cpal_device like the original implementation
        // The original only supported CPAL devices for input streams
        info!("🔍 Finding CPAL device for input: {}", device_id);
        let cpal_device = self.audio_device_manager
            .find_cpal_device(device_id, true)
            .await
            .with_context(|| format!("Failed to find CPAL input device '{}'", device_id))?;
        
        let device_name = cpal_device.name().unwrap_or_else(|_| device_id.to_string());
        info!("Found CPAL device: {}", device_name);
        
        // Get the default input config for this device
        let config = cpal_device.default_input_config()
            .with_context(|| format!("Failed to get default config for device '{}'", device_id))?;
            
        info!("Device config: {:?}", config);
        
        // **AUDIO QUALITY FIX**: Use hardware sample rate instead of fixed mixer sample rate
        let hardware_sample_rate = config.sample_rate().0;
        info!("🔧 SAMPLE RATE: Hardware {} Hz, using hardware rate to avoid resampling distortion", 
                 hardware_sample_rate);
        
        // Configure optimal buffer size for this device
        let buffer_size = self.config.buffer_size as usize;
        let optimal_buffer_size = self.calculate_optimal_buffer_size(&cpal_device, &config, buffer_size).await?;
        
        info!("🔧 BUFFER OPTIMIZATION: Using optimized buffer size {:?} for device: {}", 
              optimal_buffer_size, device_id);
        
        // Create stream config using hardware-native configuration with optimized buffer
        let stream_config = cpal::StreamConfig {
            channels: config.channels().min(2), // Limit to stereo max
            sample_rate: config.sample_rate(),   // Use hardware sample rate
            buffer_size: optimal_buffer_size,    // Use optimized buffer size
        };
        
        // Create input stream wrapper with hardware sample rate
        let input_stream = Arc::new(AudioInputStream::new(
            device_id.to_string(),
            device_name.clone(),
            hardware_sample_rate, // Use hardware sample rate instead of mixer sample rate
        )?);
        
        // **CRITICAL FIX**: Create actual CPAL stream using StreamManager
        let stream_manager = get_stream_manager();
        let (response_tx, response_rx) = std::sync::mpsc::channel();
        
        info!("🔍 Sending stream creation command to StreamManager for device: {}", device_id);
        
        // Send stream creation command to StreamManager thread
        stream_manager.send(StreamCommand::AddInputStream {
            device_id: device_id.to_string(),
            device: cpal_device,
            config: stream_config,
            audio_buffer: input_stream.audio_buffer.clone(),
            target_sample_rate: hardware_sample_rate, // Use hardware sample rate
            response_tx,
        }).with_context(|| "Failed to send stream creation command")?;
        
        // Wait for stream creation result
        match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => {
                info!("✅ CPAL STREAM: Successfully created CPAL stream for device: {}", device_id);
            },
            Ok(Err(e)) => {
                return Err(anyhow::anyhow!("Failed to create CPAL stream for '{}': {}", device_id, e));
            },
            Err(_) => {
                return Err(anyhow::anyhow!("Timeout waiting for CPAL stream creation for '{}'", device_id));
            }
        }
        
        // Initialize device health tracking
        if let Some(device_info) = self.audio_device_manager.get_device(device_id).await {
            let info = device_info;
            self.audio_device_manager.initialize_device_health(&info).await;
        }
        
        // Store the stream
        {
            let mut input_streams = self.input_streams.lock().await;
            input_streams.insert(device_id.to_string(), input_stream.clone());
        }
        
        info!("✅ INPUT STREAM: Successfully added input stream with CPAL integration for device: {}", device_id);
        Ok(())
    }

    /// Remove an input stream
    pub async fn remove_input_stream(&self, device_id: &str) -> Result<()> {
        super::validation::validate_device_id(device_id)?;
        
        info!("🔌 INPUT STREAM: Removing input stream for device: {}", device_id);
        
        // **CRITICAL FIX**: Remove CPAL stream using StreamManager
        let stream_manager = get_stream_manager();
        let (response_tx, response_rx) = std::sync::mpsc::channel();
        
        // Send stream removal command to StreamManager thread
        if let Err(e) = stream_manager.send(StreamCommand::RemoveStream {
            device_id: device_id.to_string(),
            response_tx,
        }) {
            warn!("Failed to send stream removal command for '{}': {}", device_id, e);
        } else {
            // Wait for stream removal result
            match response_rx.recv_timeout(std::time::Duration::from_secs(2)) {
                Ok(removed) => {
                    if removed {
                        info!("✅ CPAL STREAM: Successfully removed CPAL stream for device: {}", device_id);
                    } else {
                        warn!("CPAL stream for device '{}' not found", device_id);
                    }
                },
                Err(_) => {
                    warn!("Timeout waiting for CPAL stream removal for '{}'", device_id);
                }
            }
        }
        
        // Remove from input streams
        let removed_stream = {
            let mut input_streams = self.input_streams.lock().await;
            input_streams.remove(device_id)
        };
        
        if removed_stream.is_some() {
            info!("✅ INPUT STREAM: Successfully removed input stream for device: {}", device_id);
        } else {
            warn!("Input stream for device '{}' not found", device_id);
        }
        
        Ok(())
    }

    /// Set the output stream for the specified device
    pub async fn set_output_stream(&self, device_id: &str) -> Result<()> {
        super::validation::validate_device_id(device_id)?;
        
        info!("🔊 OUTPUT STREAM: Setting output stream for device: {}", device_id);
        
        // **CRITICAL FIX**: Stop existing output streams first
        info!("🔴 Stopping existing output streams before device change...");
        if let Err(e) = self.stop_output_streams().await {
            warn!("Error stopping existing output streams: {}", e);
        }
        
        // Extended delay for complete audio resource cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        // Find the audio device with fallback
        let device_handle = match self.audio_device_manager.find_audio_device(device_id, false).await {
            Ok(handle) => handle,
            Err(e) => {
                error!("Failed to find output device '{}': {}", device_id, e);
                
                // Try to refresh devices and try again
                warn!("Attempting to refresh device list and retry...");
                if let Err(refresh_err) = self.audio_device_manager.refresh_devices().await {
                    error!("Failed to refresh devices: {}", refresh_err);
                }
                
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
        
        // **CRITICAL FIX**: Handle different device types and create actual CPAL streams
        match device_handle {
            crate::audio::types::AudioDeviceHandle::Cpal(device) => {
                self.create_cpal_output_stream(device_id, device).await
            }
            #[cfg(target_os = "macos")]
            crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device) => {
                self.create_coreaudio_output_stream(device_id, coreaudio_device).await
            }
        }
    }

    /// Create CPAL output stream (restored from original implementation)
    async fn create_cpal_output_stream(&self, device_id: &str, device: cpal::Device) -> Result<()> {
        let device_name = device.name().unwrap_or_else(|_| device_id.to_string());
        info!("Found CPAL output device: {}", device_name);
        
        // Get the default output config for this device
        let config = device.default_output_config()
            .context("Failed to get default output config")?;
            
        info!("Output device config: {:?}", config);
        
        // **AUDIO QUALITY FIX**: Get hardware output sample rate to match input processing
        let hardware_output_sample_rate = config.sample_rate().0;
        info!("🔧 OUTPUT SAMPLE RATE FIX: Hardware {} Hz, using hardware rate to match input processing", 
                 hardware_output_sample_rate);
        
        // Create AudioOutputStream structure
        let output_stream = AudioOutputStream::new(
            device_id.to_string(),
            device_name.clone(),
            hardware_output_sample_rate, // Use hardware sample rate instead of mixer sample rate
        )?;
        
        // Get reference to the buffer for the output callback
        let output_buffer = output_stream.input_buffer.clone();
        let target_sample_rate = self.config.sample_rate;
        
        // Create stream config for output using hardware sample rate
        let stream_config = cpal::StreamConfig {
            channels: 2, // Force stereo output
            sample_rate: config.sample_rate(), // Use hardware sample rate, not mixer sample rate
            buffer_size: cpal::BufferSize::Default,
        };
        
        info!("Using output stream config: channels={}, sample_rate={}", 
                stream_config.channels, stream_config.sample_rate.0);
        
        // **CRITICAL FIX**: Create actual CPAL stream with audio callback
        info!("Building CPAL output stream with format: {:?}", config.sample_format());
        
        // Send stream creation command to StreamManager
        let stream_manager = get_stream_manager();
        let (response_tx, response_rx) = std::sync::mpsc::channel();
        
        stream_manager.send(StreamCommand::AddOutputStream {
            device_id: device_id.to_string(),
            device,
            config: stream_config,
            audio_buffer: output_buffer.clone(),
            response_tx,
        }).with_context(|| "Failed to send output stream creation command")?;
        
        // Wait for stream creation result
        match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => {
                info!("✅ CPAL OUTPUT STREAM: Successfully created CPAL output stream for device: {}", device_id);
            },
            Ok(Err(e)) => {
                return Err(anyhow::anyhow!("Failed to create CPAL output stream for '{}': {}", device_id, e));
            },
            Err(_) => {
                return Err(anyhow::anyhow!("Timeout waiting for CPAL output stream creation for '{}'", device_id));
            }
        }
        
        // Initialize device health tracking
        if let Some(device_info) = self.audio_device_manager.get_device(device_id).await {
            let info = device_info;
            self.audio_device_manager.initialize_device_health(&info).await;
        }
        
        // Store the stream
        {
            let mut output_stream_guard = self.output_stream.lock().await;
            *output_stream_guard = Some(Arc::new(output_stream));
        }
        
        // Track active device
        {
            let mut active_devices = self.active_output_devices.lock().await;
            active_devices.insert(device_id.to_string());
        }
        
        // **CRITICAL FIX**: Add to config.output_devices (missing from modularization)
        // Get device info first (before acquiring config lock to avoid Send trait issue)
        let device_info = self.audio_device_manager.get_device(device_id).await;
        let device_name = device_info
            .as_ref()
            .map(|info| info.name.clone())
            .unwrap_or_else(|| device_id.to_string());
        
        // Then acquire config lock and update
        {
            let mut config_guard = self.shared_config.lock().unwrap();
            
            // Create OutputDevice entry
            let output_device = crate::audio::types::OutputDevice {
                device_id: device_id.to_string(),
                device_name,
                enabled: true,
                gain: 1.0,
                is_monitor: false,
            };
            
            // Remove any existing entry for this device
            config_guard.output_devices.retain(|d| d.device_id != device_id);
            
            // Add the new entry
            config_guard.output_devices.push(output_device);
            
            info!("✅ ADDED TO CONFIG: Device '{}' added to config.output_devices", device_id);
        }
        
        info!("✅ OUTPUT STREAM: Successfully set output stream with CPAL integration for device: {}", device_id);
        Ok(())
    }

    /// Create CoreAudio output stream for direct hardware access
    #[cfg(target_os = "macos")]
    async fn create_coreaudio_output_stream(&self, device_id: &str, coreaudio_device: crate::audio::types::CoreAudioDevice) -> Result<()> {
        info!("Creating CoreAudio output stream for device: {} (ID: {})", coreaudio_device.name, coreaudio_device.device_id);
        
        // **AUDIO QUALITY FIX**: Use hardware sample rate to match input processing
        info!("🔧 COREAUDIO SAMPLE RATE FIX: Hardware {} Hz, using hardware rate to match input processing", 
              coreaudio_device.sample_rate);
        
        // Create the actual CoreAudio stream
        let mut coreaudio_stream = crate::audio::devices::coreaudio_stream::CoreAudioOutputStream::new(
            coreaudio_device.device_id,
            coreaudio_device.name.clone(),
            coreaudio_device.sample_rate, // Use hardware sample rate instead of mixer sample rate
            coreaudio_device.channels,
        )?;
        
        // Start the CoreAudio stream
        match coreaudio_stream.start() {
            Ok(()) => {
                info!("Successfully started CoreAudio stream");
            }
            Err(e) => {
                error!("Failed to start CoreAudio stream: {}", e);
                return Err(anyhow::anyhow!("Failed to start CoreAudio stream: {}", e));
            }
        }
        
        // Store the CoreAudio stream in the mixer to keep it alive
        let mut coreaudio_guard = self.coreaudio_stream.lock().await;
        *coreaudio_guard = Some(coreaudio_stream);
        
        // Create AudioOutputStream structure for compatibility
        let output_stream = AudioOutputStream::new(
            device_id.to_string(),
            coreaudio_device.name.clone(),
            coreaudio_device.sample_rate, // Use hardware sample rate instead of mixer sample rate
        )?;
        
        // Store our wrapper 
        let mut stream_guard = self.output_stream.lock().await;
        *stream_guard = Some(Arc::new(output_stream));
        
        info!("✅ Real CoreAudio output stream created and started for: {}", device_id);
        
        Ok(())
    }

    /// Stop all output streams safely
    async fn stop_output_streams(&self) -> Result<()> {
        info!("Stopping all output streams...");
        
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
            info!("Clearing {} active output devices", active_devices_guard.len());
            active_devices_guard.clear();
        }
        
        // Clear output stream
        {
            let mut output_stream_guard = self.output_stream.lock().await;
            *output_stream_guard = None;
        }
        
        // Clear output streams collection
        {
            let mut output_streams_guard = self.output_streams.lock().await;
            output_streams_guard.clear();
        }
        
        info!("✅ All output streams stopped and cleared");
        Ok(())
    }

    /// Start the audio processing thread (restored from original implementation)
    async fn start_processing_thread(&self) -> Result<()> {
        let is_running = self.is_running.clone();
        let mix_buffer = self.mix_buffer.clone();
        let audio_output_tx = self.audio_output_tx.clone();
        let audio_output_broadcast_tx = self.audio_output_broadcast_tx.clone();
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
                frame_count += 1;
                let should_log_debug = frame_count % 1000 == 0;
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
                
                // **BUFFER DEBUG**: Log buffer collection patterns
                if should_log_debug {
                    let total_samples: usize = input_samples.values().map(|v| v.len()).sum();
                    println!("🔊 BUFFER COLLECTION Frame {}: {} devices, {} total samples, channels_configured={}", 
                        frame_count, input_samples.len(), total_samples, current_channels.len());
                    
                    for (device_id, samples) in input_samples.iter() {
                        println!("  Device {}: {} samples", device_id, samples.len());
                    }
                }
                
                // If no audio data is available from callbacks, add small delay to prevent excessive CPU usage
                // **RT THREAD FIX**: Add delay to prevent overwhelming system with debug output
                if input_samples.is_empty() {
                    if should_log_debug {  // Log every 5 seconds when no input
                        println!("⚠️  NO INPUT SAMPLES: Frame {} - no audio data available from {} configured channels", 
                            frame_count, current_channels.len());
                    }
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
                                if should_log_debug && (peak_left > 0.001 || peak_right > 0.001) {
                                    crate::audio_debug!("Channel {} ({}): {} samples, L(peak: {:.3}, rms: {:.3}) R(peak: {:.3}, rms: {:.3})", 
                                        channel.id, device_id, samples.len(), peak_left, rms_left, peak_right, rms_right);
                                }
                            }
                            
                            // **AUDIO QUALITY FIX**: Use input samples directly without unnecessary conversion
                            // The input streams should already be providing stereo interleaved samples
                            // Assume input is already in the correct stereo format from stream manager
                            let stereo_samples = samples;
                            
                            // **CRITICAL FIX**: Safe buffer size matching to prevent crashes
                            // Only mix up to the smaller buffer size to prevent overruns
                            let mix_length = reusable_output_buffer.len().min(stereo_samples.len());
                            
                            // Add samples with bounds checking
                            for i in 0..mix_length {
                                if i < reusable_output_buffer.len() && i < stereo_samples.len() {
                                    reusable_output_buffer[i] += stereo_samples[i];
                                }
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
                    if should_log_debug && (left_peak > 0.001 || right_peak > 0.001) {
                        crate::audio_debug!("Master output: L(peak: {:.3}, rms: {:.3}) R(peak: {:.3}, rms: {:.3})", 
                            left_peak, left_rms, right_peak, right_rms);
                    }
                }
                
                // Store calculated channel levels for VU meters
                if !calculated_channel_levels.is_empty() {
                    if should_log_debug {
                        crate::audio_debug!("📊 STORING LEVELS: Attempting to store {} channel levels", calculated_channel_levels.len());
                        for (channel_id, (peak_left, rms_left, peak_right, rms_right)) in calculated_channel_levels.iter() {
                            crate::audio_debug!("   Level [Channel {}]: L(peak={:.4}, rms={:.4}) R(peak={:.4}, rms={:.4})", 
                                channel_id, peak_left, rms_left, peak_right, rms_right);
                        }
                    }
                    
                    match channel_levels.try_lock() {
                        Ok(mut levels_guard) => {
                            *levels_guard = calculated_channel_levels.clone();
                            if should_log_debug {
                                crate::audio_debug!("✅ STORED LEVELS: Successfully stored {} channel levels in HashMap", calculated_channel_levels.len());
                            }
                        }
                        Err(_) => {
                            if should_log_debug {
                                println!("🚫 STORAGE FAILED: Could not lock channel_levels HashMap for storage");
                            }
                        }
                    }
                } else {
                    if should_log_debug {
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
                
                // **STREAMING INTEGRATION**: Also send to broadcast channel for streaming bridge
                match audio_output_broadcast_tx.send(reusable_output_buffer.clone()) {
                    Ok(_) => {
                        if should_log_debug { // Log every ~100ms at 48kHz
                            println!("📡 Mixer broadcast: sent {} samples to {} receivers", 
                                reusable_output_buffer.len(), 
                                audio_output_broadcast_tx.receiver_count());
                        }
                    },
                    Err(tokio::sync::broadcast::error::SendError(_)) => {
                        if should_log_debug {
                            println!("📡 Mixer broadcast: no active receivers (recording/streaming stopped)");
                        }
                    }
                }
                // Don't break on send failure - just continue processing
                
                // **TIMING FIX**: Use actual samples processed instead of theoretical buffer_size
                let actual_samples_processed: usize = input_samples.values().map(|v| v.len()).sum();
                let samples_processed = if actual_samples_processed > 0 { 
                    actual_samples_processed 
                } else { 
                    0 // No samples processed when no input available
                };
                
                let processing_time_us = timing_start.elapsed().as_micros() as f64;
                let actual_input_samples = input_samples.len();
                let total_input_sample_count: usize = input_samples.values().map(|v| v.len()).sum();
                let output_buffer_size = reusable_output_buffer.len();
                
                // Log timing details every 1000 frames (about once per second at typical rates)
                if should_log_debug {
                    println!("🕐 TIMING DEBUG Frame {}: samples_processed={}, actual_inputs={}, total_input_samples={}, output_buffer={}, processing_time={:.1}μs", 
                        frame_count, samples_processed, actual_input_samples, total_input_sample_count, output_buffer_size, processing_time_us);
                }
                
                // Update audio clock with processed samples (only when samples were actually processed)
                if samples_processed > 0 {
                        if let Ok(mut clock_guard) = audio_clock.try_lock() {
                            // **CRITICAL FIX**: Use consistent hardware buffer size, not variable samples_processed
                            // BlackHole delivers 512 samples per callback, not 1024
                            let hardware_buffer_size = 512u32; // BlackHole's actual buffer size
                            let current_sync_interval = clock_guard.get_sync_interval();
                            if current_sync_interval != hardware_buffer_size as u64 {
                                println!("🔄 UPDATING AUDIOCLOCK: sync_interval {} -> {} (BlackHole hardware buffer)", 
                                    current_sync_interval, hardware_buffer_size);
                                clock_guard.set_hardware_buffer_size(hardware_buffer_size);
                            }
                            
                            // Log clock state before update
                            if should_log_debug {
                                println!("🕐 CLOCK STATE: samples_processed_before={}, sample_rate={}, sync_interval={}", 
                                    clock_guard.get_samples_processed(), clock_guard.get_sample_rate(), samples_processed);
                            }
                            
                            // Update with hardware buffer size (512) instead of accumulated samples (1024)
                            if let Some(sync_info) = clock_guard.update(hardware_buffer_size as usize) {
                                if should_log_debug {
                                    println!("🕐 TIMING SYNC: callback_interval={:.2}ms, expected={:.2}ms, variation={:.2}ms, drift_significant={}", 
                                        sync_info.callback_interval_us / 1000.0, sync_info.expected_interval_us / 1000.0, 
                                        sync_info.timing_variation / 1000.0, sync_info.is_drift_significant);
                                }
                                
                                // Clock detected timing drift - log it
                                if sync_info.is_drift_significant && should_log_debug {
                                    println!("⚠️  SIGNIFICANT TIMING DRIFT: {:.2}ms variation at {} samples ({}% of expected)", 
                                        sync_info.timing_variation / 1000.0, sync_info.samples_processed, sync_info.get_variation_percentage());
                                    
                                    // Record sync adjustment in metrics
                                    if let Ok(mut metrics_guard) = timing_metrics.try_lock() {
                                        metrics_guard.record_sync_adjustment();
                                    }
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
                        println!("📈 {}", metrics_guard.get_performance_summary());
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
                    if should_log_debug {
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

}