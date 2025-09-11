// Audio stream lifecycle management
//
// This module handles the creation, management, and cleanup of audio input
// and output streams. It coordinates device switching, stream reconfiguration,
// and ensures proper resource cleanup.

use anyhow::{Context, Result};
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
    RemoveInputStream {
        device_id: String,
        response_tx: oneshot::Sender<Result<bool>>,
    },
    #[cfg(target_os = "macos")]
    AddCoreAudioOutputStream {
        device_id: String,
        coreaudio_device: crate::audio::types::CoreAudioDevice,
        response_tx: oneshot::Sender<Result<()>>,
    },
    #[cfg(target_os = "macos")]
    AddCoreAudioInputStreamAlternative {
        device_id: String,
        coreaudio_device_id: coreaudio_sys::AudioDeviceID,
        device_name: String,
        sample_rate: u32,
        channels: u16,
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
                // DEBUG: Log what samples we're trying to write
                use std::sync::{LazyLock, Mutex as StdMutex};
                static WRITE_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                if let Ok(mut count) = WRITE_COUNT.lock() {
                    *count += 1;
                    if *count <= 10 || *count % 1000 == 0 {
                        let peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        println!("📝 SPMC_WRITE [{}]: Writing {} samples to SPMC queue, peak: {:.4}",
                            count, samples.len(), peak);
                    }
                }

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

pub struct StreamManager {
    #[cfg(target_os = "macos")]
    coreaudio_streams: HashMap<String, crate::audio::devices::CoreAudioOutputStream>,
    #[cfg(target_os = "macos")]
    coreaudio_input_streams: HashMap<String, crate::audio::devices::CoreAudioInputStream>,
}

impl std::fmt::Debug for StreamManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamManager")
            .finish()
    }
}

/// Isolated Audio Manager - owns audio streams directly, no Arc sharing!
pub struct IsolatedAudioManager {
    input_streams: HashMap<String, AudioInputStream>,
    output_spmc_writers: HashMap<String, Arc<Mutex<Writer<f32>>>>,
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


    pub fn new(command_rx: mpsc::Receiver<AudioCommand>) -> Self {
        Self {
            input_streams: HashMap::new(),
            output_spmc_writers: HashMap::new(),
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
                    static INPUT_NOTIFY_RECEIVED: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                    if let Ok(mut count) = INPUT_NOTIFY_RECEIVED.lock() {
                        *count += 1;
                        if *count <= 10 || *count % 100 == 0 {
                            println!("🔔 INPUT_NOTIFICATION_RECEIVED [{}]: Async loop got notified!", count);
                        }
                    }

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
            AudioCommand::RemoveInputStream {
                device_id,
                response_tx,
            } => {
                let result = self.handle_remove_input_stream(device_id);
                let _ = response_tx.send(Ok(result));
            }
            #[cfg(target_os = "macos")]
            AudioCommand::AddCoreAudioOutputStream {
                device_id,
                coreaudio_device,
                response_tx,
            } => {
                let result = self.add_coreaudio_output_stream_direct(device_id, coreaudio_device);
                let _ = response_tx.send(result);
            }
            #[cfg(target_os = "macos")]
            AudioCommand::AddCoreAudioInputStreamAlternative {
                device_id,
                coreaudio_device_id,
                device_name,
                sample_rate,
                channels,
                producer,
                input_notifier,
                response_tx,
            } => {
                let result = self
                    .handle_add_coreaudio_input_stream_alternative(
                        device_id,
                        coreaudio_device_id,
                        device_name,
                        sample_rate,
                        channels,
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
        static DEBUG_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
        if let Ok(mut count) = DEBUG_COUNT.lock() {
            *count += 1;
            if *count <= 10 || *count % 1000 == 0 {
                println!("🔧 PROCESS_AUDIO [{}]: Called with {} inputs, {} outputs",
                    count, self.input_streams.len(), self.output_spmc_writers.len());
            }
        }

        if self.input_streams.is_empty() {
            // Only skip if no inputs - we'll drain inputs even without outputs
            return;
        }


        // **PROFESSIONAL MIXING**: Collect samples from all input streams with effects
        let mut input_samples = Vec::<(String, Vec<f32>)>::new();

        for (device_id, input_stream) in &mut self.input_streams {
            // **EFFECTS FIX**: Create default channel config with effects enabled
            // This ensures effects are applied in IsolatedAudioManager processing
            let mut default_channel_config = crate::audio::types::AudioChannel::default();
            default_channel_config.name = format!("Channel for {}", device_id);
            default_channel_config.input_device_id = Some(device_id.clone());
            default_channel_config.effects_enabled = true; // **CRITICAL**: Enable effects processing
            
            // **EFFECTS FIX**: Use process_with_effects instead of raw get_samples
            let samples = input_stream.process_with_effects(&default_channel_config);
            if !samples.is_empty() {
                // Debug log for first few audio processing cycles
                let peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                use std::sync::{LazyLock, Mutex as StdMutex};
                static PROCESS_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                if let Ok(mut count) = PROCESS_COUNT.lock() {
                    *count += 1;
                    if *count <= 20 || *count % 1000 == 0 {
                        println!("🎵 AUDIO_PROCESSING [{}]: Input '{}' provided {} samples, peak: {:.4}",
                            count, device_id, samples.len(), peak);
                    }
                }
                
                input_samples.push((device_id.clone(), samples));
            }
        }

        // **PROFESSIONAL MIXING**: Use sophisticated mixing logic from VirtualMixer
        let mixed_samples = self.mix_input_samples_professionally(input_samples);

        if !mixed_samples.is_empty() {

            if self.output_spmc_writers.is_empty() {
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
                // Send mixed audio to all hardware output streams via SPMC queues
                for (device_id, spmc_writer) in &self.output_spmc_writers {
                    if let Ok(mut writer) = spmc_writer.try_lock() {
                        // Write samples to SPMC queue for hardware stream to read
                        for &sample in &mixed_samples {
                            writer.write(sample);
                        }

                        // Debug log for output distribution
                        use std::sync::{LazyLock, Mutex as StdMutex};
                        static OUTPUT_COUNT: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
                            LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));
                        if let Ok(mut count_map) = OUTPUT_COUNT.lock() {
                            let count = count_map.entry(device_id.clone()).or_insert(0);
                            *count += 1;
                            if *count <= 20 || *count % 1000 == 0 {
                                let peak = mixed_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                                println!("🔊 AUDIO_OUTPUT [{}]: Sent {} samples to '{}', peak: {:.4}",
                                    count, mixed_samples.len(), device_id, peak);
                            }
                        }
                    }
                }
            }

            self.metrics.total_samples_processed += mixed_samples.len() as u64;
        }
    }

    #[cfg(target_os = "macos")]
    async fn handle_add_coreaudio_input_stream_alternative(
        &mut self,
        device_id: String,
        coreaudio_device_id: coreaudio_sys::AudioDeviceID,
        device_name: String,
        sample_rate: u32,
        channels: u16,
        producer: Producer<f32>,
        input_notifier: Arc<Notify>,
    ) -> Result<()> {
        info!(
            "🎤 Adding CoreAudio input stream (CPAL alternative) for device '{}' (CoreAudio ID: {})",
            device_id, coreaudio_device_id
        );

        // **CRITICAL FIX**: Create AudioInputStream wrapper to match CPAL architecture
        // This allows get_samples_for_device() to find CoreAudio streams in input_streams
        let mut input_stream = AudioInputStream::new(
            device_id.clone(),
            device_name.clone(),
            sample_rate,
        )?;

        // Create new RTRB pair - consumer goes to AudioInputStream, producer goes to CoreAudio callback
        let buffer_capacity = (sample_rate as usize * 2) / 10; // 100ms of stereo samples
        let buffer_capacity = buffer_capacity.max(4096).min(16384);
        let (coreaudio_producer, audio_input_consumer) = rtrb::RingBuffer::<f32>::new(buffer_capacity);

        // Replace the consumer in input_stream with our CoreAudio consumer
        input_stream.audio_buffer_consumer = audio_input_consumer;

        // Store the input stream (with consumer) so get_samples_for_device() can find it
        self.input_streams.insert(device_id.clone(), input_stream);

        // Use StreamManager to create and start the CoreAudio input stream as CPAL alternative
        self.stream_manager.add_coreaudio_input_stream_alternative(
            device_id.clone(),
            coreaudio_device_id,
            device_name,
            sample_rate,
            channels,
            coreaudio_producer, // Use new producer that connects to AudioInputStream consumer
            self.global_input_notifier.clone(), // CRITICAL FIX: Use global notifier like CPAL
        )?;

        self.metrics.input_streams = self.input_streams.len();
        info!(
            "✅ CoreAudio input stream (CPAL alternative) added and started for device '{}'",
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


    #[cfg(target_os = "macos")]
    fn add_coreaudio_output_stream_direct(
        &mut self,
        device_id: String,
        coreaudio_device: crate::audio::types::CoreAudioDevice,
    ) -> Result<()> {
        info!(
            "🔊 Creating CoreAudio output stream for device '{}' (ID: {})",
            device_id, coreaudio_device.device_id
        );

        // Create SPMC queue for this output device
        let buffer_capacity = (coreaudio_device.sample_rate as usize * 2) / 10; // 100ms of stereo samples
        let buffer_capacity = buffer_capacity.max(4096).min(16384); // Clamp between 4K-16K samples

        let (spmc_reader, spmc_writer) = spmcq::ring_buffer(buffer_capacity);
        let spmc_writer = Arc::new(Mutex::new(spmc_writer));

        // Store the SPMC writer for mixer to send audio data
        self.output_spmc_writers.insert(device_id.clone(), spmc_writer);

        // Create the hardware CoreAudio stream with SPMC reader
        self.stream_manager.add_coreaudio_output_stream(
            device_id.clone(),
            coreaudio_device,
            spmc_reader,
            self.global_output_notifier.clone(),
        )?;

        self.metrics.output_streams = self.output_spmc_writers.len();
        info!(
            "✅ CoreAudio output stream created and started for device '{}' with direct SPMC connection",
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
        // TEMPORARY FIX: Disable VU meter buffer draining to test if it's stealing samples
        // VU meters were competing with process_audio() for RTRB consumer access
        HashMap::new()

        // ORIGINAL CODE (commented out for testing):
        // let mut levels = HashMap::new();
        // Get samples from each input stream and calculate VU levels
        // for (device_id, input_stream) in &mut self.input_streams {
        //     let samples = input_stream.get_samples();
        //     if !samples.is_empty() {
        //         // Calculate RMS level for VU meter
        //         let rms =
        //             (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        //         let db_level = if rms > 0.0 { 20.0 * rms.log10() } else { -60.0 };
        //         levels.insert(device_id.clone(), db_level);

        //         self.metrics.total_samples_processed += samples.len() as u64;
        //     }
        // }
        // levels
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
        // Debug removed

        if let Some(stream) = self.input_streams.get_mut(device_id) {
            let samples = if channel_config.effects_enabled {
                stream.process_with_effects(channel_config)
            } else {
                stream.get_samples()
            };
            // Debug removed to reduce log spam
            samples
        } else {
            // Debug removed to reduce log spam
            Vec::new()
        }
    }

    /// Professional audio mixing with stereo processing, smart gain management, and level calculation
    /// Based on the sophisticated VirtualMixer logic for high-quality audio
    fn mix_input_samples_professionally(
        &self,
        input_samples: Vec<(String, Vec<f32>)>, // (device_id, samples) pairs
    ) -> Vec<f32> {
        if input_samples.is_empty() {
            return Vec::new();
        }

        // Calculate required buffer size based on actual input samples
        let required_stereo_samples = input_samples.iter()
            .map(|(_, samples)| samples.len())
            .max()
            .unwrap_or(256);

        // Dynamic buffer allocation
        let mut reusable_output_buffer = vec![0.0f32; required_stereo_samples];

        // Mix all input channels together and calculate levels
        let mut active_channels = 0;

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

                // Debug log for mixing process
                use std::sync::{LazyLock, Mutex as StdMutex};
                static MIX_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                let should_log = if let Ok(mut count) = MIX_COUNT.try_lock() {
                    *count += 1;
                    *count <= 5 || *count % 1000 == 0
                } else {
                    false
                };

                if should_log && (peak_left > 0.001 || peak_right > 0.001) {
                    println!("🎛️ PROFESSIONAL_MIX: Channel '{}' - {} samples, L(peak: {:.3}, rms: {:.3}) R(peak: {:.3}, rms: {:.3})",
                        device_id, samples.len(), peak_left, rms_left, peak_right, rms_right);
                }

                // **AUDIO QUALITY FIX**: Use input samples directly without unnecessary conversion
                let stereo_samples = samples;

                // **CRITICAL FIX**: Safe buffer size matching to prevent crashes
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

        // **AUDIO QUALITY FIX**: Professional master gain instead of aggressive reduction
        let master_gain = 0.9f32; // Professional level

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

        reusable_output_buffer
    }
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            coreaudio_streams: HashMap::new(),
            #[cfg(target_os = "macos")]
            coreaudio_input_streams: HashMap::new(),
        }
    }

    /// Add CoreAudio input stream as alternative to CPAL (same interface, different backend)
    #[cfg(target_os = "macos")]
    pub fn add_coreaudio_input_stream_alternative(
        &mut self,
        device_id: String,
        coreaudio_device_id: coreaudio_sys::AudioDeviceID,
        device_name: String,
        sample_rate: u32,
        channels: u16,
        producer: Producer<f32>, // Owned RTRB Producer (exactly like CPAL)
        input_notifier: Arc<Notify>, // Event notification (exactly like CPAL)
    ) -> Result<()> {
        info!(
            "🎤 Creating CoreAudio input stream (CPAL alternative) for device '{}' (ID: {}, SR: {}Hz, CH: {})",
            device_id, coreaudio_device_id, sample_rate, channels
        );

        // Create CoreAudio input stream with RTRB producer integration (mirrors CPAL exactly)
        let mut coreaudio_input_stream =
            crate::audio::devices::CoreAudioInputStream::new_with_rtrb_producer(
                coreaudio_device_id,
                device_name.clone(),
                sample_rate,
                channels,
                producer, // Use producer provided by IsolatedAudioManager
                input_notifier,
            )?;

        // Start the CoreAudio input stream
        coreaudio_input_stream.start()?;

        // Store the CoreAudio input stream to prevent it from being dropped
        self.coreaudio_input_streams
            .insert(device_id.clone(), coreaudio_input_stream);

        info!(
            "✅ CoreAudio input stream (CPAL alternative) created and started for device '{}'",
            device_id
        );
        Ok(())
    }

    /// Remove a stream by device ID
    pub fn remove_stream(&mut self, device_id: &str) -> bool {
        let mut removed = false;


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

    /// Add output stream with SPMC Reader for lock-free audio playback (supports CoreAudio)
    pub fn add_output_stream(
        &mut self,
        device_id: String,
        device_handle: crate::audio::types::AudioDeviceHandle,
        spmc_reader: spmcq::Reader<f32>,
        output_notifier: Arc<Notify>, // Notification channel for event-driven processing
    ) -> Result<()> {
        match device_handle {
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
            "✅ CoreAudio output stream created and started for device '{}' via queue architecture (add_coreaudio_output_stream)",
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
