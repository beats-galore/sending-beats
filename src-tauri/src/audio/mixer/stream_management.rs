// Audio stream lifecycle management
//
// This module handles the creation, management, and cleanup of audio input
// and output streams. It coordinates device switching, stream reconfiguration,
// and ensures proper resource cleanup.

use anyhow::{Context, Result};
use cpal::traits::DeviceTrait;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::{error, info, warn};

use super::types::VirtualMixer;
use crate::audio::effects::{AudioEffectsChain, EQBand};
use crate::audio::types::AudioChannel;
use tokio::sync::Mutex;

// Audio stream management structures
#[derive(Debug)]
pub struct AudioInputStream {
    pub device_id: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub audio_buffer: Arc<Mutex<Vec<f32>>>,
    pub effects_chain: Arc<Mutex<AudioEffectsChain>>,
    pub adaptive_chunk_size: usize, // Adaptive buffer chunk size based on hardware
                                    // Stream is managed separately via StreamManager to avoid Send/Sync issues
}

impl AudioInputStream {
    pub fn new(device_id: String, device_name: String, sample_rate: u32) -> Result<Self> {
        let audio_buffer = Arc::new(Mutex::new(Vec::new()));
        let effects_chain = Arc::new(Mutex::new(AudioEffectsChain::new(sample_rate)));

        // Calculate optimal chunk size based on sample rate for low latency (5-10ms target)
        let optimal_chunk_size = (sample_rate as f32 * 0.005) as usize; // 5ms default

        Ok(AudioInputStream {
            device_id,
            device_name,
            sample_rate,
            channels: 2, // Fixed: Match stereo hardware (BlackHole 2CH)
            audio_buffer,
            effects_chain,
            adaptive_chunk_size: optimal_chunk_size.max(64).min(1024), // Clamp between 64-1024 samples
        })
    }

    /// Set adaptive chunk size based on hardware buffer configuration
    pub fn set_adaptive_chunk_size(&mut self, hardware_buffer_size: usize) {
        // Use hardware buffer size if reasonable, otherwise calculate optimal size
        let adaptive_size = if hardware_buffer_size > 32 && hardware_buffer_size <= 2048 {
            hardware_buffer_size
        } else {
            // Fallback: Use a reasonable default instead of hardcoded 5ms
            // Calculate based on sample rate for low latency
            let fallback_latency_ms = if self.sample_rate >= 48000 {
                5.0 // 5ms for high sample rates
            } else {
                10.0 // 10ms for lower sample rates
            };
            ((self.sample_rate as f32 * fallback_latency_ms / 1000.0) as usize)
                .max(64)  // Minimum for stability
                .min(1024) // Maximum to prevent excessive latency
        };

        self.adaptive_chunk_size = adaptive_size;
        println!(
            "🔧 ADAPTIVE BUFFER: Set chunk size to {} samples for device {}",
            self.adaptive_chunk_size, self.device_id
        );
    }

    pub fn get_samples(&self) -> Vec<f32> {
        // Track lock failures - potential cause of audio stutters
        use std::sync::atomic::{AtomicU64, Ordering};
        static LOCK_FAILURES: AtomicU64 = AtomicU64::new(0);
        if let Ok(mut buffer) = self.audio_buffer.try_lock() {
            let chunk_size = self.adaptive_chunk_size;

            if buffer.is_empty() {
                return Vec::new(); // No samples available at all
            }

            // **USE ADAPTIVE CHUNK SIZE**: Process exactly the calculated buffer size for proper timing
            let available_samples = buffer.len();
            let samples_to_take = chunk_size.min(available_samples);

            if samples_to_take == 0 {
                return Vec::new();
            }

            // Take exactly chunk_size samples for consistent timing
            let samples: Vec<f32> = buffer.drain(..samples_to_take).collect();
            let sample_count = samples.len();

            // Debug: Log when we're actually reading samples
            use std::sync::{LazyLock, Mutex as StdMutex};
            static GET_SAMPLES_COUNT: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
                LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

            if let Ok(mut count_map) = GET_SAMPLES_COUNT.lock() {
                let count = count_map.entry(self.device_id.clone()).or_insert(0);
                *count += 1;

                if sample_count > 0 {
                    if *count % 100 == 0 || (*count < 10) {
                        let peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        let rms = (samples.iter().map(|&s| s * s).sum::<f32>()
                            / samples.len() as f32)
                            .sqrt();
                        println!("📖 GET_SAMPLES [{}]: Retrieved {} samples (call #{}), peak: {:.4}, rms: {:.4}",
                            self.device_id, sample_count, count, peak, rms);
                    }
                } else if *count % 500 == 0 {
                    println!(
                        "📪 GET_SAMPLES [{}]: Empty buffer (call #{})",
                        self.device_id, count
                    );
                }
            }

            samples
        } else {

            let failures = LOCK_FAILURES.fetch_add(1, Ordering::Relaxed);

                println!("🚫 GET_SAMPLES LOCK FAILED [{}]: #{} lock failures - potential audio dropout cause",
                    self.device_id, failures);


            Vec::new()
        }
    }

    /// Apply effects to input samples and update channel settings
    pub fn process_with_effects(&self, channel: &AudioChannel) -> Vec<f32> {
        if let Ok(mut buffer) = self.audio_buffer.try_lock() {
            let chunk_size = self.adaptive_chunk_size;

            if buffer.is_empty() {
                return Vec::new(); // No samples available at all
            }

            // **USE ADAPTIVE CHUNK SIZE**: Process exactly the calculated buffer size for proper timing
            let available_samples = buffer.len();
            let samples_to_take = chunk_size.min(available_samples);

            if samples_to_take == 0 {
                return Vec::new();
            }

            // Take exactly chunk_size samples for consistent timing and effects processing
            let mut samples: Vec<f32> = buffer.drain(..samples_to_take).collect();
            let original_sample_count = samples.len();

            // Debug: Log processing activity
            use std::sync::{LazyLock, Mutex as StdMutex};
            static PROCESS_COUNT: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
                LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

            if let Ok(mut count_map) = PROCESS_COUNT.lock() {
                let count = count_map.entry(self.device_id.clone()).or_insert(0);
                *count += 1;

                if original_sample_count > 0 {
                    let original_peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);

                    if *count % 100 == 0 || (*count < 10) {
                        crate::audio_debug!("⚙️  PROCESS_WITH_EFFECTS [{}]: Processing {} samples (call #{}), peak: {:.4}, channel: {}",
                            self.device_id, original_sample_count, count, original_peak, channel.name);
                        crate::audio_debug!(
                            "   Settings: gain: {:.2}, muted: {}, effects: {}",
                            channel.gain,
                            channel.muted,
                            channel.effects_enabled
                        );
                    }
                }
            }

            // Apply effects if enabled
            if channel.effects_enabled && !samples.is_empty() {
                println!("🔊 Applying effects to samples: {}", samples.len());
                if let Ok(mut effects) = self.effects_chain.try_lock() {
                    // Update effects parameters based on channel settings
                    effects.set_eq_gain(EQBand::Low, channel.eq_low_gain);
                    effects.set_eq_gain(EQBand::Mid, channel.eq_mid_gain);
                    effects.set_eq_gain(EQBand::High, channel.eq_high_gain);

                    if channel.comp_enabled {
                        effects.set_compressor_params(
                            channel.comp_threshold,
                            channel.comp_ratio,
                            channel.comp_attack,
                            channel.comp_release,
                        );
                    }

                    if channel.limiter_enabled {
                        effects.set_limiter_threshold(channel.limiter_threshold);
                    }

                    // Process samples through effects chain
                    effects.process(&mut samples);
                }
            }

            // **CRITICAL FIX**: Apply channel-specific gain and mute (this was missing!)
            if !channel.muted && channel.gain > 0.0 {
                for sample in samples.iter_mut() {
                    *sample *= channel.gain;
                }

                // Debug: Log final processed levels
                if let Ok(count_map) = PROCESS_COUNT.lock() {
                    let count = count_map.get(&self.device_id).unwrap_or(&0);
                    if original_sample_count > 0 && (*count % 100 == 0 || *count < 10) {
                        let final_peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        let final_rms = (samples.iter().map(|&s| s * s).sum::<f32>()
                            / samples.len() as f32)
                            .sqrt();
                        crate::audio_debug!(
                            "✅ PROCESSED [{}]: Final {} samples, peak: {:.4}, rms: {:.4}",
                            self.device_id,
                            samples.len(),
                            final_peak,
                            final_rms
                        );
                    }
                }
            } else {
                samples.fill(0.0);
                if let Ok(count_map) = PROCESS_COUNT.lock() {
                    let count = count_map.get(&self.device_id).unwrap_or(&0);
                    if original_sample_count > 0 && (*count % 200 == 0 || *count < 5) {
                        println!("🔇 MUTED/ZERO_GAIN [{}]: {} samples set to silence (muted: {}, gain: {:.2})",
                            self.device_id, samples.len(), channel.muted, channel.gain);
                    }
                }
            }

            samples
        } else {
            // Track lock failures in effects processing - potential cause of audio stutters
            use std::sync::atomic::{AtomicU64, Ordering};
            static EFFECTS_LOCK_FAILURES: AtomicU64 = AtomicU64::new(0);
            let failures = EFFECTS_LOCK_FAILURES.fetch_add(1, Ordering::Relaxed);

            if failures % 100 == 0 || failures < 10 {
                println!("🚫 PROCESS_WITH_EFFECTS LOCK FAILED [{}]: #{} lock failures - potential audio dropout cause",
                    self.device_id, failures);
            }

            Vec::new()
        }
    }
}

#[derive(Debug)]
pub struct AudioOutputStream {
    pub device_id: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub input_buffer: Arc<Mutex<Vec<f32>>>,
    // Stream is handled separately to avoid Send/Sync issues
}

impl AudioOutputStream {
    pub fn new(device_id: String, device_name: String, sample_rate: u32) -> Result<Self> {
        let input_buffer = Arc::new(Mutex::new(Vec::new()));

        Ok(AudioOutputStream {
            device_id,
            device_name,
            sample_rate,
            channels: 2, // Stereo output
            input_buffer,
        })
    }

    /// Get device ID
    pub fn get_device_id(&self) -> &str {
        &self.device_id
    }

    pub fn send_samples(&self, samples: &[f32]) {
        if let Ok(mut buffer) = self.input_buffer.try_lock() {
            buffer.extend_from_slice(samples);
            // Limit buffer size to prevent memory issues
            let max_samples = self.sample_rate as usize * 2; // 2 seconds max
            let buffer_len = buffer.len();
            if buffer_len > max_samples {
                buffer.drain(0..(buffer_len - max_samples));
            }
        }
    }
}

// Stream management handles the actual cpal streams in a separate synchronous context
pub struct StreamManager {
    streams: HashMap<String, cpal::Stream>,
}

impl std::fmt::Debug for StreamManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamManager")
            .field("streams", &format!("{} streams", self.streams.len()))
            .finish()
    }
}

/// Commands that can be sent to the StreamManager thread
pub enum StreamCommand {
    AddInputStream {
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
        target_sample_rate: u32,
        response_tx: std::sync::mpsc::Sender<Result<()>>,
    },
    AddOutputStream {
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
        response_tx: std::sync::mpsc::Sender<Result<()>>,
    },
    RemoveStream {
        device_id: String,
        response_tx: std::sync::mpsc::Sender<bool>,
    },
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
        }
    }

    pub fn add_input_stream(
        &mut self,
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
        target_sample_rate: u32,
    ) -> Result<()> {
        self.add_input_stream_with_error_handling(
            device_id,
            device,
            config,
            audio_buffer,
            target_sample_rate,
            None,
        )
    }

    pub fn add_input_stream_with_error_handling(
        &mut self,
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
        target_sample_rate: u32,
        device_manager: Option<std::sync::Weak<crate::audio::devices::AudioDeviceManager>>,
    ) -> Result<()> {
        use cpal::traits::StreamTrait;
        use cpal::SampleFormat;

        // Clone device manager for error callbacks
        let _device_manager_for_errors = device_manager.clone();

        // **CRASH DEBUG**: Add detailed logging around device config retrieval
        println!(
            "🔍 CRASH DEBUG: About to get default input config for device: {}",
            device_id
        );
        let device_config = match device.default_input_config() {
            Ok(config) => {
                println!("✅ CRASH DEBUG: Successfully got device config for {}: {}Hz, {} channels, format: {:?}",
                    device_id, config.sample_rate().0, config.channels(), config.sample_format());
                config
            }
            Err(e) => {
                eprintln!(
                    "❌ CRASH DEBUG: Failed to get device config for {}: {}",
                    device_id, e
                );
                eprintln!("   This is likely the crash point - device config retrieval failed");
                return Err(anyhow::anyhow!(
                    "Device config retrieval failed for {}: {}",
                    device_id,
                    e
                ));
            }
        };

        // **CRITICAL FIX**: Use device native sample rate AND channel count to prevent conversion artifacts
        let mut native_config = config.clone();
        native_config.sample_rate = device_config.sample_rate();
        native_config.channels = device_config.channels(); // **CRASH FIX**: Use device native channel count

        println!("🔧 DEVICE NATIVE FIX: Device {} native: {}Hz, {} ch | mixer config: {}Hz, {} ch → Using native {}Hz, {} ch",
            device_id, device_config.sample_rate().0, device_config.channels(),
            config.sample_rate.0, config.channels,
            native_config.sample_rate.0, native_config.channels);

        // Add debugging context
        println!("🔍 CRASH DEBUG: About to get device name for {}", device_id);
        let device_name_for_debug = match device.name() {
            Ok(name) => {
                println!("✅ CRASH DEBUG: Device name retrieved: {}", name);
                name
            }
            Err(e) => {
                eprintln!(
                    "⚠️ CRASH DEBUG: Failed to get device name for {}: {}",
                    device_id, e
                );
                "Unknown Device".to_string()
            }
        };
        let debug_device_id = device_id.clone();
        let debug_device_id_for_callback = debug_device_id.clone();
        let debug_device_id_for_error = debug_device_id.clone();

        println!(
            "🔍 CRASH DEBUG: About to create stream with format: {:?}",
            device_config.sample_format()
        );
        let stream = match device_config.sample_format() {
            SampleFormat::F32 => {
                println!(
                    "🎤 Creating F32 input stream for: {} ({})",
                    device_name_for_debug, debug_device_id
                );
                println!(
                    "   Config: {} channels, {} Hz, {} samples/buffer",
                    native_config.channels,
                    native_config.sample_rate.0,
                    match &native_config.buffer_size {
                        cpal::BufferSize::Fixed(s) => s.to_string(),
                        cpal::BufferSize::Default => "default".to_string(),
                    }
                );

                // Debug counters
                let mut callback_count = 0u64;
                let mut total_samples_captured = 0u64;
                let _last_debug_time = std::time::Instant::now();

                println!("🔍 CRASH DEBUG: About to call device.build_input_stream for F32 format");
                let build_result = device.build_input_stream(
                    &native_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        callback_count += 1;

                        // Calculate audio levels for debugging
                        let peak_level = data.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        let rms_level = (data.iter().map(|&s| s * s).sum::<f32>() / data.len() as f32).sqrt();

                        // Keep stereo data as-is to prevent pitch shifting - don't convert to mono
                        let audio_samples: Vec<f32> = data.to_vec();

                        total_samples_captured += audio_samples.len() as u64;

                        // Debug logging every 2 seconds (approximately)
                        if callback_count % 200 == 0 || (peak_level > 0.01 && callback_count % 50 == 0) {
                            crate::audio_debug!("🔊 INPUT [{}] Callback #{}: {} samples, peak: {:.4}, rms: {:.4}",
                                debug_device_id_for_callback, callback_count, data.len(), peak_level, rms_level);
                            crate::audio_debug!("   Total samples captured: {}, stereo samples: {}", total_samples_captured, audio_samples.len());
                        }

                        // Store in buffer with additional debugging
                        if let Ok(mut buffer) = audio_buffer.try_lock() {
                            let buffer_size_before = buffer.len();
                            buffer.extend_from_slice(&audio_samples);
                            let buffer_size_after = buffer.len();

                            // Only log buffer state changes when significant or debug needed
                            if buffer_size_before == 0 && buffer_size_after > 0 && callback_count < 10 {
                                crate::audio_debug!("📦 BUFFER: First audio data stored in buffer for {}: {} samples", debug_device_id, buffer_size_after);
                            }

                            // **SIMPLE BUFFER MANAGEMENT**: Just store incoming samples, consumer drains them completely
                            // No complex overflow management needed since we process all available samples

                            // Debug buffer state periodically
                            if callback_count % 500 == 0 && buffer.len() > 0 {
                                crate::audio_debug!("📊 BUFFER STATUS [{}]: {} samples stored",
                                    debug_device_id, buffer.len());
                            }
                        } else {
                            if callback_count % 100 == 0 {
                                crate::audio_debug!("🔒 BUFFER LOCK FAILED [{}]: Callback #{} couldn't access buffer", debug_device_id, callback_count);
                            }
                        }
                    },
                    {
                        let error_device_id = debug_device_id_for_error.clone();
                        let _device_manager_weak = device_manager.clone();
                        move |err| {
                            eprintln!("❌ Audio input error [{}]: {}", error_device_id, err);

                            // Report error to device manager for health tracking
                            // Note: For now, just log the error. Full device manager integration
                            // requires a more complex async bridge which is pending implementation.
                            eprintln!("🔧 Device error reported for {}: Stream callback error", error_device_id);
                        }
                    },
                    None
                );

                match build_result {
                    Ok(stream) => {
                        println!(
                            "✅ CRASH DEBUG: Successfully built F32 input stream for {}",
                            device_id
                        );
                        stream
                    }
                    Err(e) => {
                        eprintln!(
                            "❌ CRASH DEBUG: Failed to build F32 input stream for {}: {}",
                            device_id, e
                        );
                        return Err(anyhow::anyhow!(
                            "Failed to build F32 input stream for {}: {}",
                            device_id,
                            e
                        ));
                    }
                }
            }
            SampleFormat::I16 => {
                println!(
                    "🎤 Creating I16 input stream for: {} ({})",
                    device_name_for_debug, debug_device_id
                );

                let mut callback_count = 0u64;
                let debug_device_id_i16 = debug_device_id.clone();
                let debug_device_id_i16_error = debug_device_id.clone();

                device.build_input_stream(
                    &native_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        callback_count += 1;

                        // **CRITICAL FIX**: Proper I16 to F32 conversion to prevent distortion
                        let f32_samples = crate::audio::mixer::audio_processing::AudioFormatConverter::convert_i16_to_f32_optimized(data);

                        let peak_level = f32_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        let rms_level = (f32_samples.iter().map(|&s| s * s).sum::<f32>() / f32_samples.len() as f32).sqrt();

                        // Keep stereo data as-is to prevent pitch shifting - don't convert to mono
                        let audio_samples = f32_samples;

                        if callback_count % 200 == 0 || (peak_level > 0.01 && callback_count % 50 == 0) {
                            println!("🔊 INPUT I16 [{}] Callback #{}: {} samples, peak: {:.4}, rms: {:.4}",
                                debug_device_id_i16, callback_count, data.len(), peak_level, rms_level);
                        }

                        if let Ok(mut buffer) = audio_buffer.try_lock() {
                            let buffer_size_before = buffer.len();
                            buffer.extend_from_slice(&audio_samples);

                            if buffer_size_before == 0 && buffer.len() > 0 && callback_count < 10 {
                                println!("📦 BUFFER I16: First audio data stored for {}: {} samples", debug_device_id_i16, buffer.len());
                            }


                            // **CLEANED UP**: Use centralized buffer management
                            crate::audio::mixer::audio_processing::AudioFormatConverter::manage_buffer_overflow_optimized(&mut buffer, target_sample_rate, &debug_device_id_i16, callback_count);
                        }
                    },
                    {
                        let error_device_id = debug_device_id_i16_error.clone();
                        let _device_manager_weak = device_manager.clone();
                        move |err| {
                            eprintln!("❌ Audio input error I16 [{}]: {}", error_device_id, err);

                            // Report error to device manager for health tracking
                            // Note: For now, just log the error. Full device manager integration
                            // requires a more complex async bridge which is pending implementation.
                            eprintln!("🔧 Device error reported for {}: Stream I16 callback error", error_device_id);
                        }
                    },
                    None
                )?
            }
            SampleFormat::U16 => {
                device.build_input_stream(
                    &native_config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        // **CRITICAL FIX**: Proper U16 to F32 conversion to prevent distortion
                        let f32_samples = crate::audio::mixer::audio_processing::AudioFormatConverter::convert_u16_to_f32_optimized(data);

                        // Keep stereo data as-is to prevent pitch shifting - don't convert to mono
                        let audio_samples = f32_samples;

                        if let Ok(mut buffer) = audio_buffer.try_lock() {
                            buffer.extend_from_slice(&audio_samples);

                            // **CLEANED UP**: Use centralized buffer management
                            crate::audio::mixer::audio_processing::AudioFormatConverter::manage_buffer_overflow_optimized(&mut buffer, target_sample_rate, "U16_device", 0);
                        }
                    },
                    |err| eprintln!("Audio input error: {}", err),
                    None
                )?
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported sample format: {:?}",
                    device_config.sample_format()
                ));
            }
        };

        // **CRASH FIX**: Enhanced error handling for stream.play() with device-specific diagnostics
        match stream.play() {
            Ok(()) => {
                println!(
                    "✅ Successfully started input stream for device: {} ({})",
                    device_name_for_debug, device_id
                );
                self.streams.insert(device_id, stream);
                Ok(())
            }
            Err(e) => {
                eprintln!(
                    "❌ CRITICAL: Failed to start input stream for device '{}' ({})",
                    device_id, device_name_for_debug
                );
                eprintln!(
                    "   Device config: {} Hz, {} channels, format: {:?}",
                    device_config.sample_rate().0,
                    device_config.channels(),
                    device_config.sample_format()
                );
                eprintln!(
                    "   Native config used: {} Hz, {} channels",
                    native_config.sample_rate.0, native_config.channels
                );
                eprintln!("   Error details: {}", e);

                // **CRASH FIX**: Return detailed error instead of generic context
                Err(anyhow::anyhow!(
                    "Device '{}' stream start failed - {} Hz, {} ch, format {:?}: {}",
                    device_id,
                    native_config.sample_rate.0,
                    native_config.channels,
                    device_config.sample_format(),
                    e
                ))
            }
        }
    }

    pub fn remove_stream(&mut self, device_id: &str) -> bool {
        if let Some(stream) = self.streams.remove(device_id) {
            println!("Stopping and removing stream for device: {}", device_id);
            // Stream will be automatically dropped and stopped here
            drop(stream);
            true
        } else {
            println!("Stream not found for removal: {}", device_id);
            false
        }
    }

    /// Add an output stream for playing audio (restored from original implementation)
    pub fn add_output_stream(
        &mut self,
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
    ) -> Result<()> {
        use cpal::traits::StreamTrait;

        println!("🔊 Creating output stream for device: {}", device_id);

        // Get device configuration for validation
        let device_config = match device.default_output_config() {
            Ok(config) => {
                println!(
                    "✅ Output device config for {}: {}Hz, {} channels, format: {:?}",
                    device_id,
                    config.sample_rate().0,
                    config.channels(),
                    config.sample_format()
                );
                config
            }
            Err(e) => {
                eprintln!(
                    "❌ Failed to get output device config for {}: {}",
                    device_id, e
                );
                return Err(anyhow::anyhow!("Failed to get output device config: {}", e));
            }
        };

        println!(
            "🔧 Building output stream with format: {:?}",
            device_config.sample_format()
        );

        // Create the output stream with audio callback
        let stream_result = match device_config.sample_format() {
            cpal::SampleFormat::F32 => {
                println!("Creating F32 output stream for device: {}", device_id);
                let device_id_for_error1 = device_id.clone();
                device.build_output_stream(
                    &config,
                    {
                        let audio_buffer = audio_buffer.clone();
                        let _device = device_id.clone();
                        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            // Fill output buffer with audio from our mixer
                            if let Ok(mut buffer) = audio_buffer.try_lock() {
                                let available_samples = buffer.len().min(data.len());
                                if available_samples > 0 {
                                    // Copy samples from buffer to output
                                    data[..available_samples]
                                        .copy_from_slice(&buffer[..available_samples]);
                                    // Remove used samples from buffer
                                    buffer.drain(..available_samples);
                                    // Fill remaining with silence if needed
                                    if available_samples < data.len() {
                                        data[available_samples..].fill(0.0);
                                    }
                                } else {
                                    // No audio available, output silence
                                    data.fill(0.0);
                                }
                            } else {
                                // Can't lock buffer, output silence
                                data.fill(0.0);
                            }
                        }
                    },
                    move |err| {
                        eprintln!("Output stream error for {}: {}", device_id_for_error1, err)
                    },
                    None,
                )
            }
            _ => {
                println!(
                    "Creating default format output stream for device: {}",
                    device_id
                );
                let device_id_for_error2 = device_id.clone();
                // For non-F32 formats, try to create with the device's native format
                device.build_output_stream(
                    &cpal::StreamConfig {
                        channels: config.channels,
                        sample_rate: config.sample_rate,
                        buffer_size: config.buffer_size,
                    },
                    {
                        let audio_buffer = audio_buffer.clone();
                        let _device = device_id.clone();
                        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            if let Ok(mut buffer) = audio_buffer.try_lock() {
                                let available_samples = buffer.len().min(data.len());
                                if available_samples > 0 {
                                    data[..available_samples]
                                        .copy_from_slice(&buffer[..available_samples]);
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
                        }
                    },
                    move |err| {
                        eprintln!("Output stream error for {}: {}", device_id_for_error2, err)
                    },
                    None,
                )
            }
        };

        let stream = match stream_result {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("❌ Failed to build output stream for {}: {}", device_id, e);
                return Err(anyhow::anyhow!("Failed to build output stream: {}", e));
            }
        };

        // Start the stream
        match stream.play() {
            Ok(()) => {
                println!("✅ Output stream started successfully for: {}", device_id);
            }
            Err(e) => {
                eprintln!("❌ Failed to start output stream for {}: {}", device_id, e);
                return Err(anyhow::anyhow!("Failed to start output stream: {}", e));
            }
        }

        // Store the stream to keep it alive
        self.streams.insert(device_id.clone(), stream);
        println!(
            "✅ Output stream created and stored for device: {}",
            device_id
        );

        Ok(())
    }
}

// Global stream manager instance
static STREAM_MANAGER: std::sync::OnceLock<std::sync::mpsc::Sender<StreamCommand>> =
    std::sync::OnceLock::new();

// Initialize the stream manager thread
fn init_stream_manager() -> std::sync::mpsc::Sender<StreamCommand> {
    let (tx, rx) = std::sync::mpsc::channel::<StreamCommand>();

    std::thread::spawn(move || {
        let mut manager = StreamManager::new();
        println!("Stream manager thread started");

        while let Ok(command) = rx.recv() {
            match command {
                StreamCommand::AddInputStream {
                    device_id,
                    device,
                    config,
                    audio_buffer,
                    target_sample_rate,
                    response_tx,
                } => {
                    let result = manager.add_input_stream(
                        device_id,
                        device,
                        config,
                        audio_buffer,
                        target_sample_rate,
                    );
                    let _ = response_tx.send(result);
                }
                StreamCommand::AddOutputStream {
                    device_id,
                    device,
                    config,
                    audio_buffer,
                    response_tx,
                } => {
                    let result = manager.add_output_stream(device_id, device, config, audio_buffer);
                    let _ = response_tx.send(result);
                }
                StreamCommand::RemoveStream {
                    device_id,
                    response_tx,
                } => {
                    let result = manager.remove_stream(&device_id);
                    let _ = response_tx.send(result);
                }
            }
        }

        println!("Stream manager thread stopped");
    });

    tx
}

// Get or initialize the global stream manager
pub fn get_stream_manager() -> &'static std::sync::mpsc::Sender<StreamCommand> {
    STREAM_MANAGER.get_or_init(init_stream_manager)
}

/// Information about current stream state
#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub is_running: bool,
    pub input_stream_count: usize,
    pub output_stream_count: usize,
    pub active_output_devices: Vec<String>,
}

impl StreamInfo {
    /// Check if any streams are active
    pub fn has_active_streams(&self) -> bool {
        self.input_stream_count > 0 || self.output_stream_count > 0
    }

    /// Get total stream count
    pub fn total_stream_count(&self) -> usize {
        self.input_stream_count + self.output_stream_count
    }
}
