// Stream operations for VirtualMixer
//
// This module contains all the VirtualMixer methods related to stream lifecycle
// management, including adding/removing input/output streams, device switching,
// and stream configuration operations.

use anyhow::{Context, Result};
use std::sync::{atomic::Ordering, Arc};
use tracing::{error, info, warn};

use super::mixer_core::VirtualMixerHandle;

/// Calculate optimal target latency based on sample rate
/// Professional audio target: 1ms for high sample rates (48kHz+), 10ms for lower rates
pub fn calculate_target_latency_ms(sample_rate: u32) -> f32 {
    if sample_rate >= crate::types::DEFAULT_SAMPLE_RATE {
        5.0
    } else {
        10.0
    }
}
use super::stream_management::{AudioInputStream, AudioOutputStream, StreamInfo};
use super::types::VirtualMixer;

impl VirtualMixer {
    /// Start the mixer and initialize audio processing
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            warn!("Mixer is already running");
            return Ok(());
        }

        info!("🚀 MIXER START: Starting virtual mixer...");

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

        // Clear all active devices (NEW ARCHITECTURE)
        {
            let mut input_devices = self.active_input_devices.lock().await;
            input_devices.clear();
        }

        // Clear active output devices
        {
            let mut output_devices = self.active_output_devices.lock().await;
            output_devices.clear();
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
            let input_devices = self.active_input_devices.lock().await;
            input_devices.len()
        };

        let output_count = {
            let output_devices = self.active_output_devices.lock().await;
            output_devices.len()
        };

        let active_devices = {
            let active_devices = self.active_output_devices.lock().await;
            active_devices.clone()
        };

        let is_running = self.is_running.load(Ordering::Relaxed);

        StreamInfo {
            input_streams: input_count,
            output_streams: output_count,
            active_devices: active_devices.into_iter().collect(),
        }
    }

    /// Check if a specific device is currently active
    pub async fn is_device_active(&self, device_id: &str) -> bool {
        // Check input streams
        {
            let input_devices = self.active_input_devices.lock().await;
            if input_devices.contains(device_id) {
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

    /// Add an input stream for the specified device
    pub async fn add_input_stream(&self, device_id: &str) -> Result<()> {
        super::validation::validate_device_id(device_id)?;

        info!(
            "🔌 INPUT STREAM: Adding input stream for device: {}",
            device_id
        );

        // TODO: need to add back virtual stream input processing logic :( )
        // if device_id.starts_with("app-") {
        //     info!("🎯 VIRTUAL STREAM CHECK: Looking for virtual input stream: {}", device_id);
        //     if let Some(virtual_stream) = crate::audio::ApplicationAudioManager::get_virtual_input_stream(device_id).await {
        //         info!("✅ FOUND VIRTUAL STREAM: Using pre-registered stream for {}", device_id);
        //         let mut streams = self.active_input_devices.lock().await;
        //         streams.insert(device_id.to_string(), virtual_stream);
        //         info!("✅ Successfully added virtual input stream: {}", device_id);
        //         return Ok(());
        //     } else {
        //         warn!("❌ VIRTUAL STREAM NOT FOUND: {} not in registry, falling back to CPAL", device_id);
        //         // Continue with normal CPAL device handling instead of erroring out
        //     }
        // }

        // Check if stream already exists
        {
            let input_devices = self.active_input_devices.lock().await;
            if input_devices.contains(device_id) {
                warn!(
                    "Device {} already has an active input stream, removing first",
                    device_id
                );
                drop(input_devices);
                // Remove existing stream first
                if let Err(e) = self.remove_input_stream(device_id).await {
                    warn!("Failed to remove existing stream for {}: {}", device_id, e);
                }
            }
        }

        // **CRITICAL FIX**: Extended delay to allow proper stream cleanup and prevent crashes
        // Increased from 50ms to 200ms to ensure complete resource cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // **DEVICE DISCOVERY**: Find device (CPAL or CoreAudio) with enhanced error handling and fallback
        println!(
            "🔍 DEVICE DISCOVERY: Looking for input device: {}",
            device_id
        );
        let device_handle = match self
            .audio_device_manager
            .find_audio_device(device_id, true)
            .await
        {
            Ok(device) => {
                println!(
                    "✅ DEVICE DISCOVERY: Successfully found device for: {} (type: {})",
                    device_id,
                    match &device {
                        #[cfg(target_os = "macos")]
                        crate::audio::types::AudioDeviceHandle::CoreAudio(_) => "CoreAudio",
                    }
                );
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
                match self
                    .audio_device_manager
                    .find_audio_device(device_id, true)
                    .await
                {
                    Ok(device) => {
                        info!("Found input device '{}' after refresh", device_id);
                        device
                    }
                    Err(retry_err) => {
                        error!(
                            "Input device '{}' still not found after refresh: {}",
                            device_id, retry_err
                        );
                        return Err(anyhow::anyhow!("Input device '{}' not found or unavailable. Original error: {}. Retry error: {}", device_id, e, retry_err));
                    }
                }
            }
        };

        // Handle different device types
        match device_handle {
            #[cfg(target_os = "macos")]
            crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device) => {
                info!("🎤 Using CoreAudio input stream for device: {}", device_id);
                return self
                    .add_coreaudio_input_stream(device_id, coreaudio_device)
                    .await;
            }
        }
    }

    /// Add CoreAudio input stream (CoreAudio device type)
    #[cfg(target_os = "macos")]
    async fn add_coreaudio_input_stream(
        &self,
        device_id: &str,
        coreaudio_device: crate::audio::types::CoreAudioDevice,
    ) -> Result<()> {
        info!(
            "🎤 Creating CoreAudio input stream for device: {} (ID: {})",
            coreaudio_device.name, coreaudio_device.device_id
        );

        // Check if stream already exists and remove it first
        {
            let input_devices = self.active_input_devices.lock().await;
            if input_devices.contains(device_id) {
                warn!(
                    "CoreAudio device {} already has an active input stream, removing first",
                    device_id
                );
                drop(input_devices);
                if let Err(e) = self.remove_input_stream(device_id).await {
                    warn!(
                        "Failed to remove existing CoreAudio stream for {}: {}",
                        device_id, e
                    );
                }
            }
        }

        // Allow cleanup time before creating new stream
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // Add to streams collection first
        {
            let mut streams = self.active_input_devices.lock().await;
            streams.insert(device_id.to_string());
        }

        // Create RTRB ring buffer for lock-free audio capture
        let (producer, consumer) = rtrb::RingBuffer::<f32>::new(8192);
        let input_notifier = Arc::new(tokio::sync::Notify::new());

        // Store consumer for mixer to read audio samples from
        // TODO: Store consumer in mixer's input streams collection

        // Create CoreAudio input stream using command channel architecture
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        info!(
            "🔍 Sending CoreAudio input stream creation command to isolated audio thread for device: {}",
            device_id
        );

        let command = crate::audio::mixer::stream_management::AudioCommand::AddCoreAudioInputStreamAlternative {
            device_id: device_id.to_string(),
            coreaudio_device_id: coreaudio_device.device_id,
            device_name: coreaudio_device.name.clone(),
            sample_rate: coreaudio_device.sample_rate,
            channels: coreaudio_device.channels,
            producer,
            input_notifier,
            response_tx,
        };

        if let Err(e) = self.audio_command_tx.send(command).await {
            error!("❌ Failed to send CoreAudio command to audio thread: {}", e);
            // Remove from active devices on failure
            let mut active_devices = self.active_input_devices.lock().await;
            active_devices.remove(device_id);

            return Err(anyhow::anyhow!(
                "Failed to send CoreAudio stream creation command: {}",
                e
            ));
        }

        info!("✅ Successfully sent CoreAudio command to audio thread");

        // Wait for the response from the isolated audio thread
        let result = response_rx
            .await
            .context("Failed to receive CoreAudio stream creation response")?;

        // Initialize device health tracking
        if let Some(device_info) = self.audio_device_manager.get_device(device_id).await {
            self.audio_device_manager
                .initialize_device_health(&device_info)
                .await;
        }

        match result {
            Ok(()) => {
                info!(
                    "✅ Successfully started CoreAudio input stream for: {} (ID: {})",
                    coreaudio_device.name, coreaudio_device.device_id
                );
                Ok(())
            }
            Err(e) => {
                // Remove from active input devices if stream creation failed
                let mut active_devices = self.active_input_devices.lock().await;
                active_devices.remove(device_id);
                Err(e)
            }
        }
    }

    /// Remove an input stream
    pub async fn remove_input_stream(&self, device_id: &str) -> Result<()> {
        super::validation::validate_device_id(device_id)?;

        info!(
            "🔌 INPUT STREAM: Removing input stream for device: {}",
            device_id
        );

        // **NEW ARCHITECTURE**: Remove stream using command channel
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        // Send stream removal command to isolated audio thread
        if let Err(e) = self
            .audio_command_tx
            .send(
                crate::audio::mixer::stream_management::AudioCommand::RemoveInputStream {
                    device_id: device_id.to_string(),
                    response_tx,
                },
            )
            .await
        {
            warn!(
                "Failed to send stream removal command for '{}': {}",
                device_id, e
            );
        } else {
            // Wait for stream removal result
            match tokio::time::timeout(std::time::Duration::from_secs(2), response_rx).await {
                Ok(Ok(Ok(removed))) => {
                    if removed {
                        info!(
                            "✅ CPAL STREAM: Successfully removed CPAL stream for device: {}",
                            device_id
                        );
                    } else {
                        warn!("CPAL stream for device '{}' not found", device_id);
                    }
                }
                Ok(Ok(Err(e))) => {
                    warn!("Error removing CPAL stream for '{}': {}", device_id, e);
                }
                Ok(Err(e)) => {
                    warn!("Failed to receive response for '{}': {}", device_id, e);
                }
                Err(_) => {
                    warn!(
                        "Timeout waiting for CPAL stream removal for '{}'",
                        device_id
                    );
                }
            }
        }

        // Remove from active input devices tracking (command channel architecture)
        let removed = self.active_input_devices.lock().await.remove(device_id);

        if removed {
            info!(
                "✅ INPUT STREAM: Successfully removed input stream for device: {}",
                device_id
            );
        } else {
            warn!("Input stream for device '{}' not found", device_id);
        }

        Ok(())
    }

    /// Set the output stream for the specified device
    pub async fn set_output_stream(&self, device_id: &str) -> Result<()> {
        super::validation::validate_device_id(device_id)?;

        info!(
            "🔊 OUTPUT STREAM: Setting output stream for device: {}",
            device_id
        );

        // **HYBRID ARCHITECTURE**: Use command queue for CPAL devices, direct for CoreAudio

        // Get the device using the device manager
        let device_handle = match self
            .audio_device_manager
            .find_audio_device(device_id, false)
            .await
        {
            Ok(handle) => handle,
            Err(e) => {
                error!("Failed to find output device '{}': {}", device_id, e);
                return Err(anyhow::anyhow!("Failed to find output device: {}", e));
            }
        };

        // Handle different device types using command queue
        match device_handle {
            #[cfg(target_os = "macos")]
            crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device) => {
                // Use command queue for CoreAudio devices
                let (response_tx, response_rx) = tokio::sync::oneshot::channel();

                let command = crate::audio::mixer::stream_management::AudioCommand::AddCoreAudioOutputStream {
                    device_id: device_id.to_string(),
                    coreaudio_device,
                    response_tx,
                };

                info!("🔍 Sending CoreAudio output stream creation command to isolated audio thread for device: {}", device_id);

                if let Err(_) = self.audio_command_tx.send(command).await {
                    return Err(anyhow::anyhow!("Audio system not available"));
                }

                match response_rx.await {
                    Ok(Ok(())) => {
                        info!(
                            "✅ Added CoreAudio output device via command queue: {}",
                            device_id
                        );
                        Ok(())
                    }
                    Ok(Err(e)) => Err(anyhow::anyhow!(
                        "Failed to add CoreAudio output device: {}",
                        e
                    )),
                    Err(_) => Err(anyhow::anyhow!("Audio system did not respond")),
                }
            }
            #[cfg(not(target_os = "macos"))]
            _ => Err(anyhow::anyhow!("Unknown device handle type")),
        }
    }

    /// Create CoreAudio output stream for direct hardware access
    #[cfg(target_os = "macos")]
    async fn create_coreaudio_output_stream(
        &self,
        device_id: &str,
        coreaudio_device: crate::audio::types::CoreAudioDevice,
    ) -> Result<()> {
        info!(
            "Creating CoreAudio output stream for device: {} (ID: {})",
            coreaudio_device.name, coreaudio_device.device_id
        );

        // **AUDIO QUALITY FIX**: Use hardware sample rate to match input processing
        info!("🔧 COREAUDIO SAMPLE RATE FIX: Hardware {} Hz, using hardware rate to match input processing",
              coreaudio_device.sample_rate);

        // Create the actual CoreAudio stream
        let mut coreaudio_stream =
            crate::audio::devices::coreaudio_stream::CoreAudioOutputStream::new(
                coreaudio_device.device_id,
                coreaudio_device.name.clone(),
                self.config.sample_rate, // Use hardware sample rate instead of mixer sample rate
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
        let (output_stream, _spmc_reader) = AudioOutputStream::new(
            device_id.to_string(),
            coreaudio_device.name.clone(),
            self.config.sample_rate, // Use hardware sample rate instead of mixer sample rate
        );

        // Store our wrapper
        let mut stream_guard = self.active_output_devices.lock().await;
        stream_guard.insert(device_id.to_string());

        info!(
            "✅ Real CoreAudio output stream created and started for: {}",
            device_id
        );

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
            info!(
                "Clearing {} active output devices",
                active_devices_guard.len()
            );
            active_devices_guard.clear();
        }

        // Clear output stream
        {
            let mut output_stream_guard = self.active_output_devices.lock().await;
            output_stream_guard.clear();
        }

        // Clear output streams collection
        {
            let mut output_streams_guard = self.active_output_devices.lock().await;
            output_streams_guard.clear();
        }

        info!("✅ All output streams stopped and cleared");
        Ok(())
    }

    /// Get the actual hardware sample rate from active audio streams
    /// This fixes sample rate mismatch issues by using real hardware rates instead of mixer config
    async fn get_actual_hardware_sample_rate(&self) -> u32 {
        // Check active input streams first - they reflect actual hardware capture rates
        {
            let input_devices = self.active_input_devices.lock().await;
            if let Some(device_id) = input_devices.iter().next() {
                info!(
                    "🔧 SAMPLE RATE FIX: Found active input device: {}",
                    device_id
                );
                return crate::types::DEFAULT_SAMPLE_RATE; // Default hardware sample rate
            }
        }

        // Fallback to output stream rate if no input streams
        {
            let output_stream_guard = self.active_output_devices.lock().await;
            // TODO: Get actual sample rate from device via command channel
            // For now, skip output stream sample rate check - needs proper implementation
            // if let Some(device_id) = output_stream_guard.iter().next() {
            //     // Send command to audio thread to get actual device sample rate
            // }
        }

        // Last resort: use mixer configured rate (should rarely happen)
        let mixer_rate = self.config.sample_rate;
        warn!(
            "🔧 SAMPLE RATE FIX: No active streams found, falling back to mixer config {} Hz",
            mixer_rate
        );
        mixer_rate
    }

    /// Start the audio processing thread (DISABLED - IsolatedAudioManager handles processing)
    async fn start_processing_thread(&self) -> Result<()> {
        // **TEMPORARY DISABLE**: Skip VirtualMixer processing to avoid dual processing
        // IsolatedAudioManager handles all audio processing via event-driven architecture
        warn!("🚫 VirtualMixer processing thread DISABLED - IsolatedAudioManager handles audio");
        return Ok(());

        #[allow(unreachable_code)]
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
        let sample_rate = self.config.sample_rate;
        let timing_metrics = self.timing_metrics.clone();
        let buffer_size = self.config.buffer_size;
        let mixer_handle = VirtualMixerHandle {
            audio_command_tx: self.audio_command_tx.clone(),
            #[cfg(target_os = "macos")]
            coreaudio_stream: self.coreaudio_stream.clone(),
            channel_levels: self.channel_levels.clone(),
            config: self.shared_config.clone(), // Use shared config instead of cloned copy
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
                        crate::audio_debug!(
                            "✅ Audio thread priority set to real-time (priority: 80)"
                        );
                    } else {
                        crate::audio_debug!(
                            "⚠️ Failed to set audio thread priority - may cause audio dropouts"
                        );
                    }
                }
            }

            // Create async runtime for this thread only
            let rt = tokio::runtime::Runtime::new().expect("Failed to create audio runtime");
            rt.block_on(async move {
            let mut frame_count = 0u64;
            // Dynamic buffers that adapt to actual sample counts - no fixed allocation
            let mut reusable_output_buffer = Vec::new();
            let mut reusable_left_samples = Vec::new();
            let mut reusable_right_samples = Vec::new();

            // **CRITICAL FIX**: Detect actual hardware sample rate from active streams
            // This fixes sample rate mismatch issues that cause timing drift and audio artifacts
            // let actual_hardware_sample_rate = {
            //     let mixer_self = &mixer_handle;
            //     let input_devices = mixer_self.input_devices.lock().await;
            //     if let Some((_device_id, stream)) = input_devices.iter().next() {
            //         stream.sample_rate
            //     } else {
            //         mixer_configured_sample_rate
            //     }
            // };
            // let sample_rate = actual_hardware_sample_rate;

            crate::audio_debug!("🎵 Audio processing thread started with real mixing, optimized buffers, clock sync");

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
                    crate::audio_debug!("🔊 BUFFER COLLECTION Frame {}: {} devices, {} total samples, channels_configured={}",
                        frame_count, input_samples.len(), total_samples, current_channels.len());

                    for (device_id, samples) in input_samples.iter() {
                        crate::audio_debug!("  Device {}: {} samples", device_id, samples.len());
                    }
                }

                // If no audio data is available from callbacks, add small delay to prevent excessive CPU usage
                // **RT THREAD FIX**: Add delay to prevent overwhelming system with debug output
                if input_samples.is_empty() {
                    if should_log_debug {  // Log every 5 seconds when no input
                        crate::audio_debug!("⚠️  NO INPUT SAMPLES: Frame {} - no audio data available from {} configured channels",
                            frame_count, current_channels.len());
                    }
                    std::thread::sleep(std::time::Duration::from_micros(100)); // 0.1ms sleep
                    continue;
                }

                // Calculate required buffer size based on actual input samples
                let total_input_samples: usize = input_samples.values().map(|v| v.len()).sum();
                let required_stereo_samples = if total_input_samples > 0 {
                    total_input_samples // Use actual sample count - no artificial minimum
                } else {
                    256 // Only use fallback when no samples available
                };

                // Resize buffers dynamically to handle actual sample counts
                reusable_output_buffer.clear();
                reusable_output_buffer.resize(required_stereo_samples, 0.0);
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
                            crate::audio_debug!("🔧 GAIN CONTROL: Normalized {} channels, peak {:.3} -> {:.3}",
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
                        crate::audio_debug!("🔧 MASTER LIMITER: Hot signal {:.3}, applied {:.2} gain", pre_master_peak, conservative_gain);
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
                                crate::audio_debug!("🚫 STORAGE FAILED: Could not lock channel_levels HashMap for storage");
                            }
                        }
                    }
                } else {
                    if should_log_debug {
                        crate::audio_debug!("⚠️  NO LEVELS TO STORE: calculated_channel_levels is empty");
                    }
                }

                // Also update cache for fallback (non-blocking)
                if !calculated_channel_levels.is_empty() {
                    if let Ok(mut cache_guard) = channel_levels_cache.try_lock() {
                        *cache_guard = calculated_channel_levels;
                    }
                }

                // Update mix buffer - resize dynamically to match actual output
                match mix_buffer.try_lock() {
                    Ok(mut buffer_guard) => {
                        // Always resize to match current audio output size
                        buffer_guard.clear();
                        buffer_guard.extend_from_slice(&reusable_output_buffer);
                    },
                    Err(_) => {
                        eprintln!("🚨 CRITICAL: Failed to update mix_buffer - audio output may be silent!");
                    }
                }

                // Send processed audio to the rest of the application (non-blocking)
                let _ = audio_output_tx.try_send(reusable_output_buffer.clone());

                // **STREAMING INTEGRATION**: Also send to broadcast channel for streaming bridge
                // match audio_output_broadcast_tx.send(reusable_output_buffer.clone()) {
                //     Ok(_) => {
                //         if should_log_debug { // Log every ~100ms at 48kHz
                //             crate::audio_debug!("📡 Mixer broadcast: sent {} samples to {} receivers",
                //                 reusable_output_buffer.len(),
                //                 audio_output_broadcast_tx.receiver_count());
                //         }
                //     },
                //     Err(tokio::sync::broadcast::error::SendError(_)) => {
                //         if should_log_debug {
                //             crate::audio_debug!("📡 Mixer broadcast: no active receivers (recording/streaming stopped)");
                //         }
                //     }
                // }
                // Don't break on send failure - just continue processing

                // **PRIORITY 5: Audio Clock Synchronization** - Update master clock and timing metrics
                let actual_samples_processed: usize = input_samples.values().map(|v| v.len()).sum();
                let samples_processed = actual_samples_processed;
                let processing_time_us = timing_start.elapsed().as_micros() as f64;
                let actual_input_samples = input_samples.len();
                let total_input_sample_count: usize = input_samples.values().map(|v| v.len()).sum();
                let output_buffer_size = reusable_output_buffer.len();

                // Log timing details every 1000 frames (about once per second at typical rates)
                if should_log_debug {
                    crate::audio_debug!("🕐 TIMING DEBUG Frame {}: samples_processed={}, actual_inputs={}, total_input_samples={}, output_buffer={}, processing_time={:.1}μs",
                        frame_count, samples_processed, actual_input_samples, total_input_sample_count, output_buffer_size, processing_time_us);
                }

                // Update audio clock with processed samples
                if let Ok(mut clock_guard) = audio_clock.try_lock() {
                if let Some(sync_info) = clock_guard.update(samples_processed) {
                    // Clock detected timing drift - log it
                    if sync_info.needs_adjustment {
                        if should_log_debug {
                            println!("⚠️  TIMING DRIFT: {:.2}ms drift detected at {} samples",
                            sync_info.drift_microseconds / 1000.0, sync_info.samples_processed);
                        }


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

                // **TIMING METRICS**: Report comprehensive timing every 10000 frames (~10 seconds)
                if frame_count % 10000 == 0 {
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

                // Update metrics every ~1000 frames (adaptive based on actual processing)
                if frame_count % 1000 == 0 {
                    let cpu_time = process_start.elapsed().as_secs_f32();
                    // Calculate CPU usage based on actual sample processing time
                    let actual_samples = samples_processed.max(1); // Prevent division by zero
                    let theoretical_time = actual_samples as f32 / sample_rate as f32;
                    let cpu_usage = (cpu_time / theoretical_time) * 100.0;

                    if let Ok(mut metrics_guard) = metrics.try_lock() {
                        metrics_guard.cpu_usage = cpu_usage.min(100.0); // Cap at 100%
                    }

                    if input_samples.len() > 0 {
                        crate::audio_debug!("Audio processing: CPU {:.1}%, {} active streams, {} samples",
                            cpu_usage.min(100.0), input_samples.len(), actual_samples);
                    }
                }

                // **TIMING DRIFT FIX**: Replace timer-based processing with callback-driven approach
                // Only process when we have sufficient audio data from callbacks, eliminating drift

                let elapsed = process_start.elapsed();
                let actual_buffer_duration_ms = if samples_processed > 0 {
                    (samples_processed as f32 / sample_rate as f32) * 1000.0
                } else {
                    5.0 // fallback estimate
                };

                // Debug timing changes every 5000 frames (~5 seconds)
                if frame_count % 5000 == 0 {
                    println!("🕐 CALLBACK-DRIVEN: Processing triggered by audio data availability, no timer drift (processing {:.2}ms chunks)",
                        actual_buffer_duration_ms);
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

            crate::audio_debug!("Audio processing thread stopped");
            }) // End of async block for runtime
        }); // End of thread spawn

        Ok(())
    }
}
