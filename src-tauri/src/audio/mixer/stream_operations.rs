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
        
        // Audio Clock Synchronization - Clone timing references
        let audio_clock = self.audio_clock.clone();
        let timing_metrics = self.timing_metrics.clone();
        let sample_rate = self.config.sample_rate;
        let buffer_size = self.config.buffer_size;
        let mixer_handle = super::mixer_core::VirtualMixerHandle {
            input_streams: self.input_streams.clone(),
            output_stream: self.output_stream.clone(),
            output_streams: self.output_streams.clone(),
            #[cfg(target_os = "macos")]
            coreaudio_stream: self.coreaudio_stream.clone(),
            channel_levels: self.channel_levels.clone(),
            config: self.shared_config.clone(),
        };

        // Use dedicated high-priority thread for real-time audio processing
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
                let mut reusable_left_samples: Vec<f32> = Vec::with_capacity(buffer_size as usize);
                let mut reusable_right_samples: Vec<f32> = Vec::with_capacity(buffer_size as usize);
                
                println!("🎵 Audio processing thread started with real mixing, optimized buffers, and clock synchronization");

                while is_running.load(Ordering::Relaxed) {
                    let _process_start = std::time::Instant::now();
                    
                    // Audio Clock Synchronization - Track processing timing
                    let _timing_start = std::time::Instant::now();
                    
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
                                
                                // Calculate stereo L/R peak and RMS levels for VU meters
                                let (peak_left, peak_right, rms_left, rms_right) = 
                                    super::audio_processing::AudioLevelCalculator::calculate_stereo_levels(samples);
                                
                                // Store channel levels for VU meters
                                if let Some(channel) = current_channels.iter().find(|ch| {
                                    ch.input_device_id.as_ref() == Some(device_id)
                                }) {
                                    calculated_channel_levels.insert(channel.id, (peak_left, rms_left, peak_right, rms_right));
                                }
                                
                                // Mix this channel's samples into output buffer
                                for (i, &sample) in samples.iter().enumerate() {
                                    if i < reusable_output_buffer.len() {
                                        reusable_output_buffer[i] += sample;
                                    }
                                }
                            }
                        }
                        
                        // **AUDIO QUALITY RESTORATION**: Remove aggressive gain processing that was causing crunchiness
                        // The working version didn't have this complex normalization/limiting
                        
                        // Simple mixing gain control: only reduce if multiple channels are active
                        if active_channels > 1 {
                            // Simple channel mixing gain to prevent clipping when multiple channels sum
                            let mixing_gain = 1.0 / (active_channels as f32).sqrt(); // Gentle reduction based on channel count
                            for sample in reusable_output_buffer.iter_mut() {
                                *sample *= mixing_gain;
                            }
                        }
                        
                        // Apply clean master gain from configuration (not hardcoded)
                        let config_master_gain = if let Ok(config_guard) = mixer_handle.config.try_lock() {
                            config_guard.master_gain
                        } else {
                            1.0 // Default to unity gain if can't read config
                        };
                        
                        // Apply master gain without aggressive limiting
                        for sample in reusable_output_buffer.iter_mut() {
                            *sample *= config_master_gain;
                        }
                        
                        // Calculate master levels for VU meters
                        if !reusable_output_buffer.is_empty() {
                            let (peak_left, peak_right, rms_left, rms_right) = 
                                super::audio_processing::AudioLevelCalculator::calculate_stereo_levels(&reusable_output_buffer);
                            
                            // Update master levels
                            if let Ok(mut levels) = master_levels.try_lock() {
                                *levels = (peak_left, rms_left, peak_right, rms_right);
                            }
                            
                            // Update cached master levels for UI
                            if let Ok(mut levels_cache) = master_levels_cache.try_lock() {
                                *levels_cache = (peak_left, rms_left, peak_right, rms_right);
                            }
                        }
                        
                        // Send to output streams
                        mixer_handle.send_to_output(&reusable_output_buffer).await;
                        
                        // Broadcast audio for streaming/recording
                        let _ = audio_output_broadcast_tx.send(reusable_output_buffer.clone());
                    }
                    
                    // Update channel levels atomically
                    if let Ok(mut levels) = channel_levels.try_lock() {
                        *levels = calculated_channel_levels;
                    }
                    
                    // Update cached channel levels for UI (less frequent updates)
                    if frame_count % 10 == 0 {
                        if let Ok(levels_guard) = channel_levels.try_lock() {
                            if let Ok(mut levels_cache_guard) = channel_levels_cache.try_lock() {
                                *levels_cache_guard = levels_guard.clone();
                            }
                        }
                    }
                    
                    // **CRITICAL FIX**: Set hardware buffer size on first callback to fix timing sync
                    static HARDWARE_BUFFER_SIZE_SET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                    if !HARDWARE_BUFFER_SIZE_SET.load(std::sync::atomic::Ordering::Relaxed) {
                        if let Ok(mut audio_clock_guard) = audio_clock.try_lock() {
                            let hardware_buffer_size = reusable_output_buffer.len() as u32;
                            audio_clock_guard.set_hardware_buffer_size(hardware_buffer_size);
                            HARDWARE_BUFFER_SIZE_SET.store(true, std::sync::atomic::Ordering::Relaxed);
                            info!("🔧 HARDWARE BUFFER SIZE DETECTED: {} samples ({}ms at {}Hz)", 
                                  hardware_buffer_size, 
                                  (hardware_buffer_size as f64 / sample_rate as f64) * 1000.0,
                                  sample_rate);
                        }
                    }
                    
                    // Update timing and metrics
                    if let Ok(mut audio_clock_guard) = audio_clock.try_lock() {
                        audio_clock_guard.update(reusable_output_buffer.len());
                    }
                    
                    frame_count += 1;
                }
                
                println!("🛑 Audio processing thread stopped");
            });
        });
        
        Ok(())
    }
}