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

use super::sample_rate_converter::LinearSRC;
use super::stream_operations::calculate_target_latency_ms;
use super::types::VirtualMixer;
use crate::audio::effects::{AudioEffectsChain, EQBand};
use crate::audio::types::AudioChannel;
use tokio::sync::{mpsc, oneshot, Mutex, Notify};

// Lock-free audio buffer imports
use rtrb::{Consumer, Producer, RingBuffer};
use spmcq::{ring_buffer, ReadResult, Reader, Writer};

// Command channel for isolated audio thread communication
// Cannot derive Debug because Device doesn't implement Debug
pub enum AudioCommand {
    AddInputStream {
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        target_sample_rate: u32,
        response_tx: oneshot::Sender<Result<()>>,
    },
    RemoveInputStream {
        device_id: String,
        response_tx: oneshot::Sender<Result<bool>>,
    },
    AddCPALOutputStream {
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        response_tx: oneshot::Sender<Result<()>>,
    },
    #[cfg(target_os = "macos")]
    AddCoreAudioOutputStream {
        device_id: String,
        coreaudio_device: crate::audio::types::CoreAudioDevice,
        response_tx: oneshot::Sender<Result<()>>,
    },
    #[cfg(target_os = "macos")]
    AddCoreAudioInputStream {
        device_id: String,
        coreaudio_device_id: coreaudio_sys::AudioDeviceID,
        device_name: String,
        sample_rate: u32,
        producer: Producer<f32>,
        input_notifier: Arc<Notify>,
        response_tx: oneshot::Sender<Result<()>>,
    },
    UpdateEffects {
        device_id: String,
        effects: AudioEffectsChain,
        response_tx: oneshot::Sender<Result<()>>,
    },
    GetVULevels {
        response_tx: oneshot::Sender<HashMap<String, f32>>,
    },
    GetAudioMetrics {
        response_tx: oneshot::Sender<AudioMetrics>,
    },
    GetSamples {
        device_id: String,
        channel_config: crate::audio::types::AudioChannel,
        response_tx: oneshot::Sender<Vec<f32>>,
    },
}

#[derive(Debug, Clone)]
pub struct AudioMetrics {
    pub input_streams: usize,
    pub output_streams: usize,
    pub total_samples_processed: u64,
    pub buffer_underruns: u32,
    pub average_latency_ms: f32,
}

// Audio stream management structures
#[derive(Debug)]
pub struct AudioInputStream {
    pub device_id: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub audio_buffer_consumer: Consumer<f32>, // RTRB consumer for mixer thread (owned, not shared)
    pub audio_buffer_producer: Producer<f32>, // RTRB producer for audio callback (owned, not shared)
    pub effects_chain: Arc<Mutex<AudioEffectsChain>>,
    pub adaptive_chunk_size: usize, // Adaptive buffer chunk size based on hardware
    // TRUE EVENT-DRIVEN: Notification for when audio data arrives
    pub data_available_notifier: Arc<Notify>,
    // Stream is managed separately via StreamManager to avoid Send/Sync issues
}

impl AudioInputStream {
    pub fn new(device_id: String, device_name: String, sample_rate: u32) -> Result<Self> {
        // Calculate optimal chunk size based on sample rate for low latency (5-10ms target)
        let optimal_chunk_size = (sample_rate as f32 * 0.005) as usize; // 5ms default
        let clamped_chunk_size = optimal_chunk_size.max(64).min(1024); // Clamp between 64-1024 samples

        // Create RTRB ring buffer with capacity for ~100ms of audio (larger buffer for burst handling)
        let buffer_capacity = (sample_rate as usize * 2) / 10; // 100ms of stereo samples
        let buffer_capacity = buffer_capacity.max(4096).min(16384); // Clamp between 4K-16K samples

        let (producer, consumer) = RingBuffer::<f32>::new(buffer_capacity);
        let audio_buffer_producer = producer; // Lock-free producer, owned by this stream
        let audio_buffer_consumer = consumer; // Lock-free consumer, owned by this stream
        let effects_chain = Arc::new(Mutex::new(AudioEffectsChain::new(sample_rate)));

        Ok(AudioInputStream {
            device_id,
            device_name,
            sample_rate,
            channels: 2, // Fixed: Match stereo hardware (BlackHole 2CH)
            audio_buffer_consumer,
            audio_buffer_producer,
            effects_chain,
            adaptive_chunk_size: clamped_chunk_size,
            // TRUE EVENT-DRIVEN: Initialize notification system for hardware callbacks
            data_available_notifier: Arc::new(Notify::new()),
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
            let fallback_latency_ms = if self.sample_rate >= crate::types::DEFAULT_SAMPLE_RATE {
                5.0 // 5ms for high sample rates
            } else {
                10.0 // 10ms for lower sample rates
            };
            ((self.sample_rate as f32 * fallback_latency_ms / 1000.0) as usize)
                .max(64) // Minimum for stability
                .min(1024) // Maximum to prevent excessive latency
        };

        self.adaptive_chunk_size = adaptive_size;
        println!(
            "🔧 ADAPTIVE BUFFER: Set chunk size to {} samples for device {}",
            self.adaptive_chunk_size, self.device_id
        );
    }

    pub fn get_samples(&mut self) -> Vec<f32> {
        // RTRB: True lock-free sample consumption from ring buffer (no mutex!)
        let consumer = &mut self.audio_buffer_consumer;

        let chunk_size = self.adaptive_chunk_size;

        // Check available samples in ring buffer
        let available_samples = consumer.slots();
        if available_samples == 0 {
            return Vec::new(); // No samples available
        }

        // **EVENT-DRIVEN OPTIMIZATION**: Take more samples when available to prevent accumulation
        // Use chunk_size as minimum, but take up to 3x chunk_size if available to prevent buffer buildup
        let max_samples_to_take = chunk_size * 5; // 3x adaptive chunk to prevent accumulation
        let samples_to_take = max_samples_to_take.min(available_samples);
        let mut samples = Vec::with_capacity(samples_to_take);

        // Use RTRB's read method for bulk read - TRUE LOCK-FREE!
        let mut read_count = 0;
        while read_count < samples_to_take {
            match consumer.pop() {
                Ok(sample) => {
                    samples.push(sample);
                    read_count += 1;
                }
                Err(_) => break, // No more samples available
            }
        }

        let sample_count = samples.len();

        // Debug: Log when we're reading samples
        use std::sync::{LazyLock, Mutex as StdMutex};
        // static GET_SAMPLES_COUNT: LazyLock<
        //     StdMutex<std::collections::HashMap<String, u64>>,
        // > = LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

        // if let Ok(mut count_map) = GET_SAMPLES_COUNT.lock() {
        //     let count = count_map.entry(self.device_id.clone()).or_insert(0);
        //     *count += 1;

        //     if sample_count > 0 {
        //         if *count % 100 == 0 || (*count < 10) {
        //             let peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
        //             let rms = (samples.iter().map(|&s| s * s).sum::<f32>()
        //                 / samples.len() as f32)
        //                 .sqrt();
        //             println!("📖 RTRB_GET_SAMPLES [{}]: Retrieved {} samples (call #{}), available: {}, peak: {:.4}, rms: {:.4}",
        //                 self.device_id, sample_count, count, available_samples, peak, rms);
        //         }
        //     } else if *count % 500 == 0 {
        //         println!(
        //             "📪 RTRB_GET_SAMPLES [{}]: Empty ring buffer (call #{})",
        //             self.device_id, count
        //         );
        //     }
        // }

        samples
    }

    /// Check if samples are available for processing (lock-free check)
    pub fn has_samples_available(&self) -> bool {
        // RTRB: Check available samples in the consumer queue
        self.audio_buffer_consumer.slots() > 0
    }

    /// Apply effects to input samples and update channel settings
    pub fn process_with_effects(&mut self, channel: &AudioChannel) -> Vec<f32> {
        // RTRB: True lock-free sample consumption from ring buffer (no mutex!)
        let consumer = &mut self.audio_buffer_consumer;

        let chunk_size = self.adaptive_chunk_size;
        let available_samples = consumer.slots();
        if available_samples == 0 {
            return Vec::new();
        }

        // **EVENT-DRIVEN OPTIMIZATION**: Take more samples when available to prevent accumulation
        let max_samples_to_take = chunk_size * 5; // 3x adaptive chunk to prevent accumulation
        let samples_to_take = max_samples_to_take.min(available_samples);
        let mut samples = Vec::with_capacity(samples_to_take);

        // Read samples from RTRB
        let mut read_count = 0;
        while read_count < samples_to_take {
            match consumer.pop() {
                Ok(sample) => {
                    samples.push(sample);
                    read_count += 1;
                }
                Err(_) => break,
            }
        }

        // Drop the consumer lock early to avoid holding it during effects processing
        // No need to drop - consumer is owned directly

        let original_sample_count = samples.len();
        if original_sample_count == 0 {
            return Vec::new();
        }

        // Debug: Log processing activity
        // use std::sync::{LazyLock, Mutex as StdMutex};
        // static PROCESS_COUNT: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
        //     LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

        // if let Ok(mut count_map) = PROCESS_COUNT.lock() {
        //     let count = count_map.entry(self.device_id.clone()).or_insert(0);
        //     *count += 1;

        //     if original_sample_count > 0 {
        //         let original_peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);

        //         if *count % 100 == 0 || (*count < 10) {
        //             crate::audio_debug!("⚙️  RTRB_PROCESS_WITH_EFFECTS [{}]: Processing {} samples (call #{}), available: {}, peak: {:.4}, channel: {}",
        //             self.device_id, original_sample_count, count, available_samples, original_peak, channel.name);
        //         }
        //     }
        // }

        // Apply effects if enabled
        if channel.effects_enabled && !samples.is_empty() {
            match self.effects_chain.try_lock() {
                Ok(mut effects) => {
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
                Err(_) => {
                    println!("⚠️ LOCK_CONTENTION: Failed to acquire effects chain lock for device {} during processing (effects bypassed)", self.device_id);
                    // Continue without effects processing - samples pass through unmodified
                }
            }
        }

        // Apply channel-specific gain and mute
        if !channel.muted && channel.gain > 0.0 {
            for sample in samples.iter_mut() {
                *sample *= channel.gain;
            }
        } else {
            samples.fill(0.0);
        }

        samples
    }
}

pub struct AudioOutputStream {
    pub device_id: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub spmc_writer: Arc<Mutex<Writer<f32>>>, // SPMC writer for mixer thread
    // Output stream monitoring for event-driven processing
    pub last_write_time: Arc<Mutex<std::time::Instant>>,
    pub samples_written: Arc<Mutex<u64>>,
    pub buffer_capacity_samples: usize,
    // TRUE EVENT-DRIVEN: Notification for when output streams need data
    pub output_demand_notifier: Arc<Notify>,
    // Track input sample rate for dynamic conversion
    pub input_sample_rate: Arc<Mutex<f32>>,
    // Stream is handled separately to avoid Send/Sync issues
}

// Manual Debug implementation since spmcq::Writer doesn't implement Debug
impl std::fmt::Debug for AudioOutputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioOutputStream")
            .field("device_id", &self.device_id)
            .field("device_name", &self.device_name)
            .field("sample_rate", &self.sample_rate)
            .field("channels", &self.channels)
            .field("buffer_capacity_samples", &self.buffer_capacity_samples)
            .field("spmc_writer", &"<SPMC Writer>")
            .field("last_write_time", &"<Instant>")
            .field("samples_written", &"<Counter>")
            .field("output_demand_notifier", &"<Notify>")
            .finish()
    }
}

impl AudioOutputStream {
    pub fn new(device_id: String, device_name: String, sample_rate: u32) -> (Self, Reader<f32>) {
        // Create SPMC queue with capacity for ~100ms of stereo audio
        let buffer_capacity = (sample_rate as usize * 2) / 10; // 100ms of stereo samples
        let buffer_capacity = buffer_capacity.max(4096).min(16384); // Clamp between 4K-16K samples

        let (reader, writer) = ring_buffer(buffer_capacity);

        let spmc_writer = Arc::new(Mutex::new(writer));

        let output_stream = AudioOutputStream {
            device_id,
            device_name,
            sample_rate,
            channels: 2, // Stereo output
            spmc_writer,
            // Initialize monitoring fields for event-driven processing
            last_write_time: Arc::new(Mutex::new(std::time::Instant::now())),
            samples_written: Arc::new(Mutex::new(0)),
            buffer_capacity_samples: buffer_capacity,
            // TRUE EVENT-DRIVEN: Initialize output demand notification system
            output_demand_notifier: Arc::new(Notify::new()),
            // Initialize with default 48kHz input rate, will be updated when mixed audio arrives
            input_sample_rate: Arc::new(Mutex::new(crate::types::DEFAULT_SAMPLE_RATE as f32)),
        };

        (output_stream, reader)
    }

    /// Get device ID
    pub fn get_device_id(&self) -> &str {
        &self.device_id
    }

    pub fn send_samples(&self, samples: &[f32], input_sample_rate: f32) {
        // Update the input sample rate for dynamic conversion
        if let Ok(mut rate) = self.input_sample_rate.try_lock() {
            *rate = input_sample_rate;
        }

        match self.spmc_writer.try_lock() {
            Ok(mut writer) => {
                // Push samples to SPMC queue - all consumers will receive them
                let mut pushed_count = 0;
                for &sample in samples {
                    writer.write(sample);
                    pushed_count += 1;
                }

                // **EVENT-DRIVEN MONITORING**: Track write operations for demand detection
                match (
                    self.last_write_time.try_lock(),
                    self.samples_written.try_lock(),
                ) {
                    (Ok(mut last_write), Ok(mut samples_written)) => {
                        *last_write = std::time::Instant::now();
                        *samples_written = samples_written.wrapping_add(pushed_count as u64);
                    }
                    (Err(_), Ok(_)) => {
                        println!("⚠️ LOCK_CONTENTION: Failed to acquire last_write_time lock for device {}", self.device_id);
                    }
                    (Ok(_), Err(_)) => {
                        println!("⚠️ LOCK_CONTENTION: Failed to acquire samples_written lock for device {}", self.device_id);
                    }
                    (Err(_), Err(_)) => {
                        println!("⚠️ LOCK_CONTENTION: Failed to acquire both monitoring locks for device {}", self.device_id);
                    }
                }

                // Log if we couldn't write all samples (unlikely with proper sizing)
                if pushed_count < samples.len() {
                    crate::audio_debug!(
                        "⚠️ SPMC_OUTPUT_PARTIAL: Only wrote {} of {} samples to device {}",
                        pushed_count,
                        samples.len(),
                        self.device_id
                    );
                }
            }
            Err(_) => {
                println!("⚠️ LOCK_CONTENTION: Failed to acquire SPMC writer lock for device {} (dropping {} samples)",
                    self.device_id, samples.len());
            }
        }
    }
}

// Stream management handles the actual cpal streams in a separate synchronous context
pub struct StreamManager {
    streams: HashMap<String, cpal::Stream>,
    #[cfg(target_os = "macos")]
    coreaudio_streams: HashMap<String, crate::audio::devices::CoreAudioOutputStream>,
    #[cfg(target_os = "macos")]
    coreaudio_input_streams: HashMap<String, crate::audio::devices::CoreAudioInputStream>,
}

impl std::fmt::Debug for StreamManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamManager")
            .field("streams", &format!("{} streams", self.streams.len()))
            .finish()
    }
}

/// Isolated Audio Manager - owns audio streams directly, no Arc sharing!
pub struct IsolatedAudioManager {
    input_streams: HashMap<String, AudioInputStream>,
    output_streams: HashMap<String, AudioOutputStream>,
    stream_manager: StreamManager,
    command_rx: mpsc::Receiver<AudioCommand>,
    metrics: AudioMetrics,
    // TRUE EVENT-DRIVEN: Global notification channels for async processing
    global_input_notifier: Arc<Notify>,
    global_output_notifier: Arc<Notify>,
}

impl IsolatedAudioManager {
    /// Check if any input streams have data available for processing
    /// Returns true if at least one stream has samples ready
    fn has_input_data_available(&self) -> bool {
        for input_stream in self.input_streams.values() {
            // RTRB: Check if consumer has samples available (lock-free!)
            if input_stream.has_samples_available() {
                return true;
            }
        }
        false
    }

    /// Check if any output streams need more data (are running low)
    /// Returns true if any output buffer is below threshold
    fn output_streams_need_data(&self) -> bool {
        // **TRUE OUTPUT EVENT-DRIVEN DETECTION**
        // Use write timing analysis to determine if outputs need more data

        if self.output_streams.is_empty() {
            return false; // No output streams to service
        }

        let now = std::time::Instant::now();

        // Calculate expected consumption rate for real-time audio
        // Streams should consume ~sample_rate samples per second in real-time
        for output_stream in self.output_streams.values() {
            match output_stream.last_write_time.try_lock() {
                Ok(last_write_time) => {
                    let time_since_last_write = now.duration_since(*last_write_time);

                    // **CONSUMPTION RATE ANALYSIS**:
                    // In real-time audio, we should write every 1-10ms depending on sample rate
                    let target_latency_ms =
                        crate::audio::mixer::stream_operations::calculate_target_latency_ms(
                            output_stream.sample_rate,
                        );
                    let max_acceptable_gap =
                        std::time::Duration::from_millis(target_latency_ms as u64 * 2); // 2x target latency

                    // If we haven't written in too long, outputs probably need data
                    if time_since_last_write > max_acceptable_gap {
                        // **STARVATION DETECTION**: Output hasn't been fed recently
                        return true;
                    }

                    // **CONTINUOUS PROCESSING**: For real-time audio, we want regular feeding
                    // If we have inputs available and it's been more than target latency, process
                    let min_feed_interval =
                        std::time::Duration::from_millis(target_latency_ms as u64);
                    if time_since_last_write > min_feed_interval && self.has_input_data_available()
                    {
                        return true;
                    }
                }
                Err(_) => {
                    // **LOCK_CONTENTION**: Can't access timing info, be conservative and assume need data
                    println!("⚠️ LOCK_CONTENTION: Failed to acquire last_write_time lock for output stream analysis {}", output_stream.device_id);
                    return true;
                }
            }
        }

        // **DEFAULT POLICY**: If timing looks good but we have input data, process it
        // This prevents accumulation and maintains low latency
        self.has_input_data_available()
    }

    /// Calculate optimal processing interval based on active streams' sample rates
    fn calculate_processing_interval_ms(&self) -> f32 {
        // Find highest sample rate among all active streams
        let max_input_rate = self
            .input_streams
            .values()
            .map(|stream| stream.sample_rate)
            .max()
            .unwrap_or(0);

        let max_output_rate = self
            .output_streams
            .values()
            .map(|stream| stream.sample_rate)
            .max()
            .unwrap_or(0);

        let max_sample_rate = max_input_rate.max(max_output_rate);

        // Default to conservative interval when no streams
        if max_sample_rate == 0 {
            return 5.0; // 5ms default when no active streams
        }

        // Use same logic as calculate_optimal_buffer_size
        calculate_target_latency_ms(max_sample_rate)
    }

    pub fn new(command_rx: mpsc::Receiver<AudioCommand>) -> Self {
        Self {
            input_streams: HashMap::new(),
            output_streams: HashMap::new(),
            stream_manager: StreamManager::new(),
            command_rx,
            metrics: AudioMetrics {
                input_streams: 0,
                output_streams: 0,
                total_samples_processed: 0,
                buffer_underruns: 0,
                average_latency_ms: 0.0,
            },
            // TRUE EVENT-DRIVEN: Initialize global notification channels
            global_input_notifier: Arc::new(Notify::new()),
            global_output_notifier: Arc::new(Notify::new()),
        }
    }

    /// Main processing loop for the isolated audio thread
    pub async fn run(&mut self) {
        info!("🎵 Isolated audio manager started - lock-free RTRB architecture");

        // **TRUE EVENT-DRIVEN PROCESSING**: Use async notifications instead of polling
        info!("🚀 TRUE EVENT-DRIVEN: Starting async notification-driven audio processing");

        loop {
            tokio::select! {
                // Handle commands (highest priority)
                command = self.command_rx.recv() => {
                    match command {
                        Some(cmd) => {
                            self.handle_command(cmd).await;
                        },
                        None => break, // Channel closed
                    }
                }

                // **TRUE EVENT-DRIVEN**: Process when input data notification arrives
                _ = self.global_input_notifier.notified() => {
                    // DEBUG: Track that we received the notification
                    use std::sync::{LazyLock, Mutex as StdMutex};
                    // static INPUT_NOTIFY_RECEIVED: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                    // if let Ok(mut count) = INPUT_NOTIFY_RECEIVED.lock() {
                    //     *count += 1;
                    //     if *count <= 10 || *count % 100 == 0 {
                    //         info!("🔔 INPUT_NOTIFICATION_RECEIVED [{}]: Async loop got notified!", count);
                    //     }
                    // }

                    // **ALWAYS CONSUME**: Always drain input buffers to prevent overflow
                    // Process even without outputs (dummy sink behavior)
                    self.process_audio().await;

                    // // Track event-driven processing
                    // static INPUT_EVENT_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                    // if let Ok(mut count) = INPUT_EVENT_COUNT.lock() {
                    //     *count += 1;
                    //     if *count <= 5 || *count % 100 == 0 {
                    //         let output_status = if self.output_streams.is_empty() { "DUMMY_SINK" } else { "REAL_OUTPUT" };
                    //         info!("⚡ INPUT_EVENT [{}]: Processed audio on input data notification ({})", count, output_status);
                    //     }
                    // }
                }

                // **TRUE EVENT-DRIVEN**: Process when output demand notification arrives
                _ = self.global_output_notifier.notified() => {
                    if self.has_input_data_available() {
                        // **RESPONSIVE PROCESSING**: Output needs data and input has it
                        self.process_audio().await;

                        // Track event-driven processing
                        // use std::sync::{LazyLock, Mutex as StdMutex};
                        // static OUTPUT_EVENT_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                        // if let Ok(mut count) = OUTPUT_EVENT_COUNT.lock() {
                        //     *count += 1;
                        //     if *count <= 5 || *count % 1000 == 0 {
                        //         info!("⚡ OUTPUT_EVENT [{}]: Processed audio on output demand notification", count);
                        //     }
                        // }
                    }
                }


            }
        }
    }

    async fn handle_command(&mut self, command: AudioCommand) {
        match command {
            AudioCommand::AddInputStream {
                device_id,
                device,
                config,
                target_sample_rate,
                response_tx,
            } => {
                let result = self
                    .handle_add_input_stream(device_id, device, config, target_sample_rate)
                    .await;
                let _ = response_tx.send(result);
            }
            AudioCommand::RemoveInputStream {
                device_id,
                response_tx,
            } => {
                let result = self.handle_remove_input_stream(device_id);
                let _ = response_tx.send(Ok(result));
            }
            AudioCommand::AddCPALOutputStream {
                device_id,
                device,
                config,
                response_tx,
            } => {
                let result = self
                    .handle_add_cpal_output_stream(device_id, device, config)
                    .await;
                let _ = response_tx.send(result);
            }
            #[cfg(target_os = "macos")]
            AudioCommand::AddCoreAudioOutputStream {
                device_id,
                coreaudio_device,
                response_tx,
            } => {
                let result = self
                    .handle_add_coreaudio_output_stream(device_id, coreaudio_device)
                    .await;
                let _ = response_tx.send(result);
            }
            #[cfg(target_os = "macos")]
            AudioCommand::AddCoreAudioInputStream {
                device_id,
                coreaudio_device_id,
                device_name,
                sample_rate,
                producer,
                input_notifier,
                response_tx,
            } => {
                let result = self
                    .handle_add_coreaudio_input_stream(
                        device_id,
                        coreaudio_device_id,
                        device_name,
                        sample_rate,
                        producer,
                        input_notifier,
                    )
                    .await;
                let _ = response_tx.send(result);
            }
            AudioCommand::UpdateEffects {
                device_id,
                effects,
                response_tx,
            } => {
                let result = self.handle_update_effects(device_id, effects);
                let _ = response_tx.send(result);
            }
            AudioCommand::GetVULevels { response_tx } => {
                let levels = self.get_vu_levels();
                let _ = response_tx.send(levels);
            }
            AudioCommand::GetAudioMetrics { response_tx } => {
                let metrics = self.get_metrics();
                let _ = response_tx.send(metrics);
            }
            AudioCommand::GetSamples {
                device_id,
                channel_config,
                response_tx,
            } => {
                let samples = self.get_samples_for_device(&device_id, &channel_config);
                let _ = response_tx.send(samples);
            }
        }
    }

    /// Continuous audio processing: mix inputs and distribute to outputs
    async fn process_audio(&mut self) {
        // Debug: Log the processing attempt
        use std::sync::{LazyLock, Mutex as StdMutex};
        // static DEBUG_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
        // if let Ok(mut count) = DEBUG_COUNT.lock() {
        //     *count += 1;
        //     if *count <= 10 || *count % 1000 == 0 {
        //         info!("🔧 PROCESS_AUDIO [{}]: Called with {} inputs, {} outputs",
        //             count, self.input_streams.len(), self.output_streams.len());
        //     }
        // }

        if self.input_streams.is_empty() {
            // Only skip if no inputs - we'll drain inputs even without outputs
            return;
        }

        // Collect samples from all input streams
        let mut mixed_samples = Vec::<f32>::new();
        let mut active_inputs = 0;
        let mut mixed_sample_rate = crate::types::DEFAULT_SAMPLE_RATE as f32; // Default, will be set to first active input's rate

        for (device_id, input_stream) in &mut self.input_streams {
            let samples = input_stream.get_samples();
            if samples.is_empty() {
                continue;
            }
            // Debug log for first few audio processing cycles (before moving samples)
            let peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
            use std::sync::{LazyLock, Mutex as StdMutex};
            // static PROCESS_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
            // if let Ok(mut count) = PROCESS_COUNT.lock() {
            //     *count += 1;
            //     if *count <= 10 || *count % 1000 == 0 {
            //         info!("🎵 AUDIO_PROCESSING [{}]: Input '{}' provided {} samples, peak: {:.4}",
            //             count, device_id, samples.len(), peak);
            //     }
            // }

            if mixed_samples.is_empty() {
                // First input stream - initialize the mix buffer and set sample rate
                mixed_samples = samples;
                mixed_sample_rate = input_stream.sample_rate as f32;
            } else {
                // Mix additional streams by adding samples
                // Handle different lengths by extending if needed
                if samples.len() > mixed_samples.len() {
                    mixed_samples.resize(samples.len(), 0.0);
                }

                for (i, &sample) in samples.iter().enumerate() {
                    if i < mixed_samples.len() {
                        mixed_samples[i] += sample;
                    }
                }
            }
            active_inputs += 1;
        }

        if !mixed_samples.is_empty() && active_inputs > 0 {
            // Normalize by number of active inputs to prevent clipping
            if active_inputs > 1 {
                let scale = 1.0 / active_inputs as f32;
                for sample in &mut mixed_samples {
                    *sample *= scale;
                }
            }

            if self.output_streams.is_empty() {
                // **DUMMY SINK**: Consume input samples even without outputs to prevent overflow
                // use std::sync::{LazyLock, Mutex as StdMutex};
                // static DUMMY_SINK_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                // if let Ok(mut count) = DUMMY_SINK_COUNT.lock() {
                //     *count += 1;
                //     if *count <= 5 || *count % 1000 == 0 {
                //         let peak = mixed_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                //         info!("🗑️ DUMMY_SINK [{}]: Drained {} samples (no outputs), peak: {:.4}",
                //             count, mixed_samples.len(), peak);
                //     }
            } else {
                // Send mixed audio to all output streams
                for (device_id, output_stream) in &self.output_streams {
                    output_stream.send_samples(&mixed_samples, mixed_sample_rate);

                    // Debug log for output distribution
                    // use std::sync::{LazyLock, Mutex as StdMutex};
                    // static OUTPUT_COUNT: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
                    //     LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));
                    // if let Ok(mut count_map) = OUTPUT_COUNT.lock() {
                    //     let count = count_map.entry(device_id.clone()).or_insert(0);
                    //     *count += 1;
                    //     if *count <= 10 || *count % 1000 == 0 {
                    //         let peak = mixed_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    //         info!("🔊 AUDIO_OUTPUT [{}]: Sent {} samples to '{}', peak: {:.4}",
                    //             count, mixed_samples.len(), device_id, peak);
                    //     }
                    // }
                }
            }

            self.metrics.total_samples_processed += mixed_samples.len() as u64;
        }
    }

    async fn handle_add_input_stream(
        &mut self,
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        target_sample_rate: u32,
    ) -> Result<()> {
        // Create AudioInputStream with RTRB Producer/Consumer pair
        let mut input_stream = AudioInputStream::new(
            device_id.clone(),
            device.name().unwrap_or_default(),
            target_sample_rate,
        )?;

        // Extract the producer for the CPAL callback (consumer stays with input_stream)
        // We need to create a new RTRB pair because we can't split the existing one
        let buffer_capacity = (target_sample_rate as usize * 2) / 10; // 100ms of stereo samples
        let buffer_capacity = buffer_capacity.max(4096).min(16384);
        let (producer, consumer) = rtrb::RingBuffer::<f32>::new(buffer_capacity);

        // Replace the consumer in input_stream with our new one
        input_stream.audio_buffer_consumer = consumer;

        // Store the input stream (with consumer) in our owned collection
        self.input_streams.insert(device_id.clone(), input_stream);

        // Set up the actual CPAL audio stream with the producer
        self.stream_manager.add_input_stream(
            device_id.clone(),
            device,
            config,
            producer,
            target_sample_rate,
            self.global_input_notifier.clone(),
        )?;

        self.metrics.input_streams = self.input_streams.len();
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn handle_add_coreaudio_input_stream(
        &mut self,
        device_id: String,
        coreaudio_device_id: coreaudio_sys::AudioDeviceID,
        device_name: String,
        sample_rate: u32,
        producer: Producer<f32>,
        input_notifier: Arc<Notify>,
    ) -> Result<()> {
        info!(
            "🎤 Adding CoreAudio input stream for device '{}' (CoreAudio ID: {})",
            device_id, coreaudio_device_id
        );

        // Use StreamManager to create and start the CoreAudio input stream
        self.stream_manager.add_coreaudio_input_stream(
            device_id.clone(),
            coreaudio_device_id,
            device_name,
            sample_rate,
            producer,
            input_notifier,
        )?;

        self.metrics.input_streams = self.input_streams.len();
        info!(
            "✅ CoreAudio input stream added and started for device '{}'",
            device_id
        );
        Ok(())
    }

    fn handle_remove_input_stream(&mut self, device_id: String) -> bool {
        let removed = self.input_streams.remove(&device_id).is_some();
        self.stream_manager.remove_stream(&device_id);
        self.metrics.input_streams = self.input_streams.len();
        removed
    }

    async fn handle_add_cpal_output_stream(
        &mut self,
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
    ) -> Result<()> {
        info!(
            "🔊 Creating CPAL output stream for device '{}' with config: {}Hz, {} channels",
            device_id, config.sample_rate.0, config.channels
        );

        let (output_stream, spmc_reader) = AudioOutputStream::new(
            device_id.clone(),
            device.name().unwrap_or_default(),
            config.sample_rate.0,
        );

        self.output_streams.insert(device_id.clone(), output_stream);

        // Use unified method with AudioDeviceHandle
        let device_handle = crate::audio::types::AudioDeviceHandle::Cpal(device);
        self.stream_manager.add_output_stream(
            device_id.clone(),
            device_handle,
            spmc_reader,
            self.global_output_notifier.clone(),
        )?;

        self.metrics.output_streams = self.output_streams.len();
        info!(
            "✅ CPAL output stream created and started for device '{}'",
            device_id
        );
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn handle_add_coreaudio_output_stream(
        &mut self,
        device_id: String,
        coreaudio_device: crate::audio::types::CoreAudioDevice,
    ) -> Result<()> {
        info!(
            "🔊 Creating CoreAudio output stream for device '{}' (ID: {})",
            device_id, coreaudio_device.device_id
        );

        // **NEW QUEUE ARCHITECTURE**: Create output stream with SPMC queue integration
        let (output_stream, spmc_reader) = AudioOutputStream::new(
            device_id.clone(),
            coreaudio_device.name.clone(),
            coreaudio_device.sample_rate,
        );

        // Store the output stream
        self.output_streams.insert(device_id.clone(), output_stream);

        // **QUEUE INTEGRATION**: Use unified StreamManager method with CoreAudio device handle
        let device_handle = crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device);
        self.stream_manager.add_output_stream(
            device_id.clone(),
            device_handle,
            spmc_reader,
            self.global_output_notifier.clone(),
        )?;

        self.metrics.output_streams = self.output_streams.len();
        info!(
            "✅ CoreAudio output stream created and started for device '{}' via queue architecture",
            device_id
        );
        Ok(())
    }

    fn handle_update_effects(
        &mut self,
        device_id: String,
        effects: AudioEffectsChain,
    ) -> Result<()> {
        if let Some(input_stream) = self.input_streams.get_mut(&device_id) {
            match input_stream.effects_chain.try_lock() {
                Ok(mut effects_guard) => {
                    *effects_guard = effects;
                    Ok(())
                }
                Err(_) => {
                    println!(
                        "⚠️ LOCK_CONTENTION: Failed to acquire effects chain lock for device {}",
                        device_id
                    );
                    // Continue without updating effects - operation succeeds but effects update is skipped
                    Ok(())
                }
            }
        } else {
            Err(anyhow::anyhow!("Input stream not found: {}", device_id))
        }
    }

    fn get_vu_levels(&mut self) -> HashMap<String, f32> {
        let mut levels = HashMap::new();

        // Get samples from each input stream and calculate VU levels
        for (device_id, input_stream) in &mut self.input_streams {
            let samples = input_stream.get_samples();
            if !samples.is_empty() {
                // Calculate RMS level for VU meter
                let rms =
                    (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
                let db_level = if rms > 0.0 { 20.0 * rms.log10() } else { -60.0 };
                levels.insert(device_id.clone(), db_level);

                self.metrics.total_samples_processed += samples.len() as u64;
            }
        }

        levels
    }

    fn get_metrics(&self) -> AudioMetrics {
        self.metrics.clone()
    }

    /// Get processed samples from a specific device using lock-free RTRB queues
    fn get_samples_for_device(
        &mut self,
        device_id: &str,
        channel_config: &crate::audio::types::AudioChannel,
    ) -> Vec<f32> {
        if let Some(stream) = self.input_streams.get_mut(device_id) {
            // Use the lock-free RTRB implementation that's already working
            if channel_config.effects_enabled {
                stream.process_with_effects(channel_config)
            } else {
                stream.get_samples()
            }
        } else {
            Vec::new()
        }
    }
}

// Legacy StreamCommand enum removed - using AudioCommand instead

impl StreamManager {
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
            #[cfg(target_os = "macos")]
            coreaudio_streams: HashMap::new(),
            #[cfg(target_os = "macos")]
            coreaudio_input_streams: HashMap::new(),
        }
    }

    /// Add CPAL input stream with RTRB Producer for lock-free audio capture
    pub fn add_input_stream(
        &mut self,
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        producer: Producer<f32>, // Owned RTRB Producer (not Arc - avoids Send+Sync issues)
        target_sample_rate: u32,
        input_notifier: Arc<Notify>, // Notification channel for event-driven processing
    ) -> Result<()> {
        use cpal::traits::{DeviceTrait, StreamTrait};
        use cpal::SampleFormat;

        // Get device's default format to determine sample format
        let supported_config = device
            .default_input_config()
            .map_err(|e| anyhow::anyhow!("Failed to get default input config: {}", e))?;
        let sample_format = supported_config.sample_format();
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate.0;

        info!(
            "🎤 Creating input stream for device '{}' with config: {}Hz, {} channels, format: {:?}",
            device_id, sample_rate, channels, sample_format
        );

        // Move producer into the callback closure (owned, not shared)
        let mut producer = producer;
        let device_id_for_f32_callback = device_id.clone();
        let device_id_for_f32_error = device_id.clone();
        let device_id_for_i16_callback = device_id.clone();
        let device_id_for_i16_error = device_id.clone();
        // Clone notifier for callback closures
        let f32_notifier = input_notifier.clone();
        let i16_notifier = input_notifier.clone();

        // Create the audio stream with appropriate callback based on sample format
        let stream = match sample_format {
            SampleFormat::F32 => {
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                        // RTRB: Lock-free audio capture directly from hardware callback
                        let mut samples_written = 0;
                        let mut samples_dropped = 0;

                        for &sample in data.iter() {
                            match producer.push(sample) {
                                Ok(()) => samples_written += 1,
                                Err(_) => {
                                    samples_dropped += 1;
                                    // Ring buffer full - skip this sample (prevents blocking)
                                }
                            }
                        }

                        // **TRUE EVENT-DRIVEN**: Always notify async processing thread when hardware callback runs
                        // This ensures we drain the buffer even when it's full
                        f32_notifier.notify_one();

                        // DEBUG: Track notification sends
                        // use std::sync::{LazyLock, Mutex as StdMutex};
                        // static NOTIFY_SEND_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                        // if let Ok(mut count) = NOTIFY_SEND_COUNT.lock() {
                        //     *count += 1;
                        //     if *count % 100 == 0 || *count <= 5 {
                        //         println!("🚨 F32_NOTIFICATION_SENT [{}]: Hardware callback sent notification (wrote: {}, dropped: {})",
                        //             count, samples_written, samples_dropped);
                        //     }
                        // }

                        // Debug logging for audio capture
                        // static CAPTURE_COUNT: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
                        //     LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

                        // if let Ok(mut count_map) = CAPTURE_COUNT.lock() {
                        //     let count = count_map.entry(device_id_for_f32_callback.clone()).or_insert(0);
                        //     *count += 1;

                        //     if *count % 100 == 0 || (*count < 10) {
                        //         let peak = data.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        //         let rms = (data.iter().map(|&s| s * s).sum::<f32>() / data.len() as f32).sqrt();
                        //         println!("🎵 RTRB_CAPTURE [{}]: Captured {} samples (call #{}), wrote: {}, dropped: {}, peak: {:.4}, rms: {:.4} ⚡NOTIFIED",
                        //             device_id_for_f32_callback, data.len(), count, samples_written, samples_dropped, peak, rms);
                        //     }
                        // }
                    },
                    move |err| {
                        error!(
                            "❌ Input stream error for device '{}': {}",
                            device_id_for_f32_error, err
                        );
                    },
                    None, // Timeout - use default
                )?
            }
            SampleFormat::I16 => {
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _info: &cpal::InputCallbackInfo| {
                        // Convert i16 to f32 and push to RTRB
                        let mut samples_written = 0;
                        let mut samples_dropped = 0;

                        for &sample in data.iter() {
                            // Convert i16 to f32 (-1.0 to 1.0 range)
                            let f32_sample = sample as f32 / 32768.0;
                            match producer.push(f32_sample) {
                                Ok(()) => samples_written += 1,
                                Err(_) => samples_dropped += 1,
                            }
                        }

                        // **TRUE EVENT-DRIVEN**: Always notify async processing thread when hardware callback runs
                        i16_notifier.notify_one();

                        // Debug logging for audio capture
                        // use std::sync::{LazyLock, Mutex as StdMutex};
                        // static CAPTURE_COUNT_I16: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
                        //     LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

                        // if let Ok(mut count_map) = CAPTURE_COUNT_I16.lock() {
                        //     let count = count_map.entry(device_id_for_i16_callback.clone()).or_insert(0);
                        //     *count += 1;

                        //     if *count % 100 == 0 || (*count < 10) {
                        //         let peak = data.iter().map(|&s| (s as f32 / 32768.0).abs()).fold(0.0f32, f32::max);
                        //         println!("🎵 RTRB_CAPTURE_I16 [{}]: Captured {} samples (call #{}), wrote: {}, dropped: {}, peak: {:.4} ⚡NOTIFIED",
                        //             device_id_for_i16_callback, data.len(), count, samples_written, samples_dropped, peak);
                        //     }
                        // }
                    },
                    move |err| {
                        error!(
                            "❌ Input stream error for device '{}': {}",
                            device_id_for_i16_error, err
                        );
                    },
                    None, // Timeout - use default
                )?
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported sample format: {:?}",
                    sample_format
                ));
            }
        };

        // Start the stream
        stream.play()?;

        // Store the stream
        self.streams.insert(device_id.clone(), stream);

        info!(
            "✅ Input stream created and started for device '{}'",
            device_id
        );
        Ok(())
    }

    /// Add CoreAudio input stream with RTRB Producer for lock-free audio capture
    #[cfg(target_os = "macos")]
    pub fn add_coreaudio_input_stream(
        &mut self,
        device_id: String,
        coreaudio_device_id: coreaudio_sys::AudioDeviceID,
        device_name: String,
        sample_rate: u32,
        producer: Producer<f32>, // Owned RTRB Producer for lock-free audio capture
        input_notifier: Arc<Notify>, // Notification channel for event-driven processing
    ) -> Result<()> {
        info!(
            "🎤 Creating CoreAudio input stream for device '{}' (CoreAudio ID: {}, SR: {}Hz)",
            device_id, coreaudio_device_id, sample_rate
        );

        // Create CoreAudio input stream with RTRB producer integration
        let mut coreaudio_input_stream =
            crate::audio::devices::CoreAudioInputStream::new_with_rtrb_producer(
                coreaudio_device_id,
                device_name.clone(),
                sample_rate,
                2, // Stereo channels
                producer,
                input_notifier,
            )?;

        // Start the CoreAudio input stream
        coreaudio_input_stream.start()?;

        // Store the CoreAudio input stream to prevent it from being dropped
        self.coreaudio_input_streams
            .insert(device_id.clone(), coreaudio_input_stream);

        info!(
            "✅ CoreAudio input stream created and started for device '{}'",
            device_id
        );
        Ok(())
    }

    /// Remove a stream by device ID
    pub fn remove_stream(&mut self, device_id: &str) -> bool {
        let mut removed = false;

        // Try to remove CPAL stream first
        if let Some(stream) = self.streams.remove(device_id) {
            println!(
                "Stopping and removing CPAL stream for device: {}",
                device_id
            );
            drop(stream);
            removed = true;
        }

        // Try to remove CoreAudio output stream on macOS
        #[cfg(target_os = "macos")]
        {
            if let Some(mut coreaudio_stream) = self.coreaudio_streams.remove(device_id) {
                println!(
                    "Stopping and removing CoreAudio output stream for device: {}",
                    device_id
                );
                // Explicitly stop the CoreAudio stream before dropping
                if let Err(e) = coreaudio_stream.stop() {
                    eprintln!(
                        "Warning: Failed to stop CoreAudio output stream {}: {}",
                        device_id, e
                    );
                }
                drop(coreaudio_stream);
                removed = true;
            }
        }

        // Try to remove CoreAudio input stream on macOS
        #[cfg(target_os = "macos")]
        {
            if let Some(mut coreaudio_input_stream) = self.coreaudio_input_streams.remove(device_id) {
                println!(
                    "Stopping and removing CoreAudio input stream for device: {}",
                    device_id
                );
                // Explicitly stop the CoreAudio input stream before dropping
                if let Err(e) = coreaudio_input_stream.stop() {
                    eprintln!(
                        "Warning: Failed to stop CoreAudio input stream {}: {}",
                        device_id, e
                    );
                }
                drop(coreaudio_input_stream);
                removed = true;
            }
        }

        if !removed {
            println!("Stream not found for removal: {}", device_id);
        }

        removed
    }

    /// Add output stream with SPMC Reader for lock-free audio playback (supports both CPAL and CoreAudio)
    pub fn add_output_stream(
        &mut self,
        device_id: String,
        device_handle: crate::audio::types::AudioDeviceHandle,
        spmc_reader: spmcq::Reader<f32>,
        output_notifier: Arc<Notify>, // Notification channel for event-driven processing
    ) -> Result<()> {
        // Handle both CPAL and CoreAudio devices through unified queue architecture
        match device_handle {
            crate::audio::types::AudioDeviceHandle::Cpal(device) => {
                self.add_cpal_output_stream(device_id, device, spmc_reader, output_notifier)
            }
            #[cfg(target_os = "macos")]
            crate::audio::types::AudioDeviceHandle::CoreAudio(coreaudio_device) => self
                .add_coreaudio_output_stream(
                    device_id,
                    coreaudio_device,
                    spmc_reader,
                    output_notifier,
                ),
            #[cfg(not(target_os = "macos"))]
            _ => Err(anyhow::anyhow!("Unsupported device type for this platform")),
        }
    }

    /// Add CPAL output stream with SPMC Reader for lock-free audio playback
    fn add_cpal_output_stream(
        &mut self,
        device_id: String,
        device: cpal::Device,
        mut spmc_reader: spmcq::Reader<f32>,
        output_notifier: Arc<Notify>, // Notification channel for event-driven processing
    ) -> Result<()> {
        use cpal::traits::{DeviceTrait, StreamTrait};
        use cpal::SampleFormat;

        // Get device's default format to determine sample format
        let supported_config = device
            .default_output_config()
            .map_err(|e| anyhow::anyhow!("Failed to get default output config: {}", e))?;
        let sample_format = supported_config.sample_format();
        let config = supported_config.config();
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate.0;

        info!("🔊 Creating CPAL output stream for device '{}' with config: {}Hz, {} channels, format: {:?}",
            device_id, sample_rate, channels, sample_format);

        let device_id_for_f32_out_callback = device_id.clone();
        let device_id_for_f32_out_error = device_id.clone();
        let device_id_for_i16_out_callback = device_id.clone();
        let device_id_for_i16_out_error = device_id.clone();
        // Clone notifier for callback closures
        let f32_output_notifier = output_notifier.clone();
        let i16_output_notifier = output_notifier.clone();

        // Clone sample rate info for callbacks
        let f32_output_sample_rate = sample_rate as f32;
        let i16_output_sample_rate = sample_rate as f32;

        // Create the output stream with appropriate callback based on sample format
        let stream = match sample_format {
            SampleFormat::F32 => {
                device.build_output_stream(
                    &config,
                    move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                        // **DYNAMIC SAMPLE RATE CONVERSION**: Convert available input samples to exact output buffer size
                        use std::cell::RefCell;
                        thread_local! {
                            static SRC: RefCell<Option<LinearSRC>> = RefCell::new(None);
                        }

                        // Collect all available samples from SPMC queue
                        let mut input_samples = Vec::new();
                        loop {
                            match spmc_reader.read() {
                                spmcq::ReadResult::Ok(sample) => {
                                    input_samples.push(sample);
                                    // Prevent unbounded reads by limiting to reasonable chunk size
                                    if input_samples.len() >= 4096 {
                                        break;
                                    }
                                }
                                spmcq::ReadResult::Dropout(sample) => {
                                    // Got data but missed some samples, still use the audio
                                    input_samples.push(sample);
                                    if input_samples.len() >= 4096 {
                                        break;
                                    }
                                }
                                spmcq::ReadResult::Empty => {
                                    // No more samples available
                                    break;
                                }
                            }
                        }

                        if !input_samples.is_empty() {
                            // Dynamic SRC initialization - detect sample rate from input samples
                            let converted_samples = SRC.with(|src_cell| {
                                let mut src_opt = src_cell.borrow_mut();

                                // Estimate input sample rate based on sample count and time
                                // This is a heuristic approach since we can't easily pass rate info to callbacks
                                let estimated_input_rate = {
                                    // Most common audio sample rates
                                    let common_rates = crate::types::COMMON_SAMPLE_RATES_HZ;

                                    // Use heuristic: if we have ~1024 samples and output wants ~1114,
                                    // that suggests 44.1kHz input to 48kHz output (1024 * 48000/44100 ≈ 1114)
                                    let ratio_hint = data.len() as f32 / input_samples.len() as f32;
                                    let estimated_output_rate = f32_output_sample_rate;
                                    let estimated_input_rate = estimated_output_rate / ratio_hint;

                                    // Find the closest common sample rate
                                    common_rates
                                        .iter()
                                        .min_by(|&a, &b| {
                                            (a - estimated_input_rate)
                                                .abs()
                                                .partial_cmp(&(b - estimated_input_rate).abs())
                                                .unwrap()
                                        })
                                        .copied()
                                        .unwrap_or(44100.0)
                                };

                                // Reinitialize SRC if rate changed significantly
                                let needs_new_src = if let Some(ref src) = *src_opt {
                                    (src.ratio() - (f32_output_sample_rate / estimated_input_rate))
                                        .abs()
                                        > 0.01
                                } else {
                                    true
                                };

                                if needs_new_src {
                                    *src_opt = Some(LinearSRC::new(
                                        estimated_input_rate,
                                        f32_output_sample_rate,
                                    ));
                                }

                                if let Some(ref mut src) = *src_opt {
                                    src.convert(&input_samples, data.len())
                                } else {
                                    vec![0.0; data.len()]
                                }
                            });

                            // Copy converted samples to output buffer
                            for (i, &sample) in converted_samples.iter().enumerate() {
                                if i < data.len() {
                                    data[i] = sample;
                                }
                            }
                        } else {
                            // No input samples available - fill with silence
                            data.fill(0.0);
                        }

                        // **TRUE EVENT-DRIVEN**: Notify async processing thread when we need more samples
                        if input_samples.is_empty() {
                            f32_output_notifier.notify_one();
                        }

                        // Debug logging for audio playback
                        // use std::sync::{LazyLock, Mutex as StdMutex};
                        // static PLAYBACK_COUNT: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
                        //     LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

                        // if let Ok(mut count_map) = PLAYBACK_COUNT.lock() {
                        //     let count = count_map.entry(device_id_for_f32_out_callback.clone()).or_insert(0);
                        //     *count += 1;

                        //     if *count % 100 == 0 || (*count < 10) {
                        //         let peak = data.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        //         let rms = (data.iter().map(|&s| s * s).sum::<f32>() / data.len() as f32).sqrt();
                        //         let status = if input_samples.is_empty() { " ⏳SRC_UNDERRUN" } else { " ✅SRC_CONVERTED" };
                        //         println!("🎵 SPMC_PLAYBACK [{}]: Requested {} samples (call #{}), input: {}, peak: {:.4}, rms: {:.4}{}",
                        //             device_id_for_f32_out_callback, data.len(), count, input_samples.len(), peak, rms, status);
                        //     }
                        // }
                    },
                    move |err| {
                        error!(
                            "❌ Output stream error for device '{}': {}",
                            device_id_for_f32_out_error, err
                        );
                    },
                    None, // Timeout - use default
                )?
            }
            SampleFormat::I16 => {
                device.build_output_stream(
                    &config,
                    move |data: &mut [i16], _info: &cpal::OutputCallbackInfo| {
                        // **DYNAMIC SAMPLE RATE CONVERSION**: Convert available input samples to exact output buffer size
                        use std::cell::RefCell;
                        thread_local! {
                            static SRC: RefCell<Option<LinearSRC>> = RefCell::new(None);
                        }

                        // Collect all available samples from SPMC queue
                        let mut input_samples = Vec::new();
                        loop {
                            match spmc_reader.read() {
                                spmcq::ReadResult::Ok(sample) => {
                                    input_samples.push(sample);
                                    // Prevent unbounded reads by limiting to reasonable chunk size
                                    if input_samples.len() >= 4096 {
                                        break;
                                    }
                                }
                                spmcq::ReadResult::Dropout(sample) => {
                                    // Got data but missed some samples, still use the audio
                                    input_samples.push(sample);
                                    if input_samples.len() >= 4096 {
                                        break;
                                    }
                                }
                                spmcq::ReadResult::Empty => {
                                    // No more samples available
                                    break;
                                }
                            }
                        }

                        if !input_samples.is_empty() {
                            // Dynamic SRC initialization - detect sample rate from input samples
                            let converted_samples = SRC.with(|src_cell| {
                                let mut src_opt = src_cell.borrow_mut();

                                // Estimate input sample rate based on sample count and time
                                let estimated_input_rate = {
                                    // Most common audio sample rates
                                    let common_rates = crate::types::COMMON_SAMPLE_RATES_HZ;

                                    // Use heuristic: if we have ~1024 samples and output wants ~1114,
                                    // that suggests 44.1kHz input to 48kHz output (1024 * 48000/44100 ≈ 1114)
                                    let ratio_hint = data.len() as f32 / input_samples.len() as f32;
                                    let estimated_output_rate = i16_output_sample_rate;
                                    let estimated_input_rate = estimated_output_rate / ratio_hint;

                                    // Find the closest common sample rate
                                    common_rates
                                        .iter()
                                        .min_by(|&a, &b| {
                                            (a - estimated_input_rate)
                                                .abs()
                                                .partial_cmp(&(b - estimated_input_rate).abs())
                                                .unwrap()
                                        })
                                        .copied()
                                        .unwrap_or(44100.0)
                                };

                                // Reinitialize SRC if rate changed significantly
                                let needs_new_src = if let Some(ref src) = *src_opt {
                                    (src.ratio() - (i16_output_sample_rate / estimated_input_rate))
                                        .abs()
                                        > 0.01
                                } else {
                                    true
                                };

                                if needs_new_src {
                                    *src_opt = Some(LinearSRC::new(
                                        estimated_input_rate,
                                        i16_output_sample_rate,
                                    ));
                                }

                                if let Some(ref mut src) = *src_opt {
                                    src.convert(&input_samples, data.len())
                                } else {
                                    vec![0.0; data.len()]
                                }
                            });

                            // Convert f32 samples to i16 and copy to output buffer
                            for (i, &f32_sample) in converted_samples.iter().enumerate() {
                                if i < data.len() {
                                    data[i] = (f32_sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                                }
                            }
                        } else {
                            // No input samples available - fill with silence
                            data.fill(0);
                        }

                        // **TRUE EVENT-DRIVEN**: Notify async processing thread when we need more samples
                        if input_samples.is_empty() {
                            i16_output_notifier.notify_one();
                        }

                        // Debug logging for i16 playback
                        // use std::sync::{LazyLock, Mutex as StdMutex};
                        // static PLAYBACK_COUNT_I16: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
                        //     LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

                        // if let Ok(mut count_map) = PLAYBACK_COUNT_I16.lock() {
                        //     let count = count_map.entry(device_id_for_i16_out_callback.clone()).or_insert(0);
                        //     *count += 1;

                        //     if *count % 100 == 0 || (*count < 10) {
                        //         let peak = data.iter().map(|&s| (s as f32 / 32767.0).abs()).fold(0.0f32, f32::max);
                        //         let status = if input_samples.is_empty() { " ⏳SRC_UNDERRUN" } else { " ✅SRC_CONVERTED" };
                        //         println!("🎵 SPMC_PLAYBACK_I16 [{}]: Requested {} samples (call #{}), input: {}, peak: {:.4}{}",
                        //             device_id_for_i16_out_callback, data.len(), count, input_samples.len(), peak, status);
                        //     }
                        // }
                    },
                    move |err| {
                        error!(
                            "❌ Output stream error for device '{}': {}",
                            device_id_for_i16_out_error, err
                        );
                    },
                    None, // Timeout - use default
                )?
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported sample format: {:?}",
                    sample_format
                ));
            }
        };

        // Start the stream
        stream.play()?;

        // Store the stream
        self.streams.insert(device_id.clone(), stream);

        info!(
            "✅ CPAL output stream created and started for device '{}'",
            device_id
        );
        Ok(())
    }

    /// Add CoreAudio output stream with SPMC Reader integration
    #[cfg(target_os = "macos")]
    fn add_coreaudio_output_stream(
        &mut self,
        device_id: String,
        coreaudio_device: crate::audio::types::CoreAudioDevice,
        spmc_reader: spmcq::Reader<f32>,
        _output_notifier: Arc<Notify>, // CoreAudio integration pending
    ) -> Result<()> {
        info!(
            "🔊 Creating CoreAudio output stream for device '{}' (ID: {})",
            device_id, coreaudio_device.device_id
        );

        // **SPMC INTEGRATION**: Create CoreAudio stream with SPMC reader
        // Extract values from CoreAudioDevice and use new constructor
        let mut coreaudio_stream =
            crate::audio::devices::CoreAudioOutputStream::new_with_spmc_reader(
                coreaudio_device.device_id,    // AudioDeviceID (u32)
                coreaudio_device.name.clone(), // String
                coreaudio_device.sample_rate,  // u32
                2,                             // channels: u16 (stereo)
                spmc_reader,                   // **SPMC READER INTEGRATION**
            )?;

        // **SPMC READER NOW INTEGRATED**: Stream created with SPMC reader for real audio data

        // Start the CoreAudio stream
        coreaudio_stream.start()?;

        // **CRITICAL FIX**: Store the CoreAudio stream to prevent it from being dropped
        self.coreaudio_streams
            .insert(device_id.clone(), coreaudio_stream);

        info!(
            "🎵 CoreAudio stream started with SPMC queue integration for device '{}'",
            device_id
        );
        info!("🔒 CoreAudio stream stored in StreamManager to prevent premature cleanup");

        info!(
            "✅ CoreAudio output stream created and started for device '{}' via queue architecture",
            device_id
        );
        Ok(())
    }
}

/// Information about current stream state
#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub input_streams: usize,
    pub output_streams: usize,
    pub active_devices: std::collections::HashSet<String>,
}

impl StreamInfo {
    pub fn new() -> Self {
        Self {
            input_streams: 0,
            output_streams: 0,
            active_devices: std::collections::HashSet::new(),
        }
    }

    pub fn has_active_streams(&self) -> bool {
        self.input_streams > 0 || self.output_streams > 0
    }
}
