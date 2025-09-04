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
use tokio::sync::{Mutex, mpsc, oneshot};

// Lock-free audio buffer imports
use rtrb::{RingBuffer, Consumer, Producer};
use spmcq::{ring_buffer, Reader, Writer, ReadResult};

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
    AddOutputStream {
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
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
        
        // Take up to chunk_size samples for consistent timing
        let samples_to_take = chunk_size.min(available_samples);
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
        static GET_SAMPLES_COUNT: LazyLock<
            StdMutex<std::collections::HashMap<String, u64>>,
        > = LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

        if let Ok(mut count_map) = GET_SAMPLES_COUNT.lock() {
            let count = count_map.entry(self.device_id.clone()).or_insert(0);
            *count += 1;

            if sample_count > 0 {
                if *count % 100 == 0 || (*count < 10) {
                    let peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let rms = (samples.iter().map(|&s| s * s).sum::<f32>()
                        / samples.len() as f32)
                        .sqrt();
                    println!("📖 RTRB_GET_SAMPLES [{}]: Retrieved {} samples (call #{}), available: {}, peak: {:.4}, rms: {:.4}",
                        self.device_id, sample_count, count, available_samples, peak, rms);
                }
            } else if *count % 500 == 0 {
                println!(
                    "📪 RTRB_GET_SAMPLES [{}]: Empty ring buffer (call #{})",
                    self.device_id, count
                );
            }
        }
        
        samples
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
        
        // Take up to chunk_size samples for consistent timing
        let samples_to_take = chunk_size.min(available_samples);
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
        use std::sync::{LazyLock, Mutex as StdMutex};
        static PROCESS_COUNT: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
            LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

        if let Ok(mut count_map) = PROCESS_COUNT.lock() {
            let count = count_map.entry(self.device_id.clone()).or_insert(0);
            *count += 1;

            if original_sample_count > 0 {
                let original_peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);

                if *count % 100 == 0 || (*count < 10) {
                    crate::audio_debug!("⚙️  RTRB_PROCESS_WITH_EFFECTS [{}]: Processing {} samples (call #{}), available: {}, peak: {:.4}, channel: {}",
                    self.device_id, original_sample_count, count, available_samples, original_peak, channel.name);
                }
            }
        }

        // Apply effects if enabled
        if channel.effects_enabled && !samples.is_empty() {
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
            .field("spmc_writer", &"<SPMC Writer>")
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
        };
        
        (output_stream, reader)
    }

    /// Get device ID
    pub fn get_device_id(&self) -> &str {
        &self.device_id
    }

    pub fn send_samples(&self, samples: &[f32]) {
        if let Ok(mut writer) = self.spmc_writer.try_lock() {
            // Push samples to SPMC queue - all consumers will receive them
            let mut pushed_count = 0;
            for &sample in samples {
                writer.write(sample);
                pushed_count += 1;
            }
            
            // Log if we couldn't write all samples (unlikely with proper sizing)
            if pushed_count < samples.len() {
                crate::audio_debug!("⚠️ SPMC_OUTPUT_PARTIAL: Only wrote {} of {} samples to device {}", 
                    pushed_count, samples.len(), self.device_id);
            }
        } else {
            crate::audio_debug!("⚠️ SPMC_OUTPUT_LOCK_BUSY: Writer lock busy for device {}", self.device_id);
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

/// Isolated Audio Manager - owns audio streams directly, no Arc sharing!
pub struct IsolatedAudioManager {
    input_streams: HashMap<String, AudioInputStream>,
    output_streams: HashMap<String, AudioOutputStream>, 
    stream_manager: StreamManager,
    command_rx: mpsc::Receiver<AudioCommand>,
    metrics: AudioMetrics,
}

impl IsolatedAudioManager {
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
        }
    }

    /// Main processing loop for the isolated audio thread
    pub async fn run(&mut self) {
        info!("🎵 Isolated audio manager started - lock-free RTRB architecture");
        
        // Start continuous audio processing task
        let mut audio_processing_interval = tokio::time::interval(tokio::time::Duration::from_millis(1)); // ~1kHz processing rate
        
        loop {
            tokio::select! {
                // Handle commands
                command = self.command_rx.recv() => {
                    match command {
                        Some(cmd) => self.handle_command(cmd).await,
                        None => break, // Channel closed
                    }
                }
                
                // Process audio continuously  
                _ = audio_processing_interval.tick() => {
                    self.process_audio().await;
                }
            }
        }
    }
    
    async fn handle_command(&mut self, command: AudioCommand) {
        match command {
            AudioCommand::AddInputStream { device_id, device, config, target_sample_rate, response_tx } => {
                let result = self.handle_add_input_stream(device_id, device, config, target_sample_rate).await;
                let _ = response_tx.send(result);
            }
            AudioCommand::RemoveInputStream { device_id, response_tx } => {
                let result = self.handle_remove_input_stream(device_id);
                let _ = response_tx.send(Ok(result));
            }
            AudioCommand::AddOutputStream { device_id, device, config, response_tx } => {
                let result = self.handle_add_output_stream(device_id, device, config).await;
                let _ = response_tx.send(result);
            }
            AudioCommand::UpdateEffects { device_id, effects, response_tx } => {
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
            AudioCommand::GetSamples { device_id, channel_config, response_tx } => {
                let samples = self.get_samples_for_device(&device_id, &channel_config);
                let _ = response_tx.send(samples);
            }
        }
    }
    
    /// Continuous audio processing: mix inputs and distribute to outputs
    async fn process_audio(&mut self) {
        if self.input_streams.is_empty() || self.output_streams.is_empty() {
            return; // Nothing to process
        }
        
        // Collect samples from all input streams
        let mut mixed_samples = Vec::<f32>::new();
        let mut active_inputs = 0;
        
        for (device_id, input_stream) in &mut self.input_streams {
            let samples = input_stream.get_samples();
            if !samples.is_empty() {
                // Debug log for first few audio processing cycles (before moving samples)
                let peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                use std::sync::{LazyLock, Mutex as StdMutex};
                static PROCESS_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                if let Ok(mut count) = PROCESS_COUNT.lock() {
                    *count += 1;
                    if *count <= 10 || *count % 1000 == 0 {
                        info!("🎵 AUDIO_PROCESSING [{}]: Input '{}' provided {} samples, peak: {:.4}", 
                            count, device_id, samples.len(), peak);
                    }
                }
                
                if mixed_samples.is_empty() {
                    // First input stream - initialize the mix buffer
                    mixed_samples = samples;
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
        }
        
        if !mixed_samples.is_empty() && active_inputs > 0 {
            // Normalize by number of active inputs to prevent clipping
            if active_inputs > 1 {
                let scale = 1.0 / active_inputs as f32;
                for sample in &mut mixed_samples {
                    *sample *= scale;
                }
            }
            
            // Send mixed audio to all output streams
            for (device_id, output_stream) in &self.output_streams {
                output_stream.send_samples(&mixed_samples);
                
                // Debug log for output distribution
                use std::sync::{LazyLock, Mutex as StdMutex};
                static OUTPUT_COUNT: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
                    LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));
                if let Ok(mut count_map) = OUTPUT_COUNT.lock() {
                    let count = count_map.entry(device_id.clone()).or_insert(0);
                    *count += 1;
                    if *count <= 10 || *count % 1000 == 0 {
                        let peak = mixed_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        info!("🔊 AUDIO_OUTPUT [{}]: Sent {} samples to '{}', peak: {:.4}", 
                            count, mixed_samples.len(), device_id, peak);
                    }
                }
            }
            
            self.metrics.total_samples_processed += mixed_samples.len() as u64;
        }
    }

    async fn handle_add_input_stream(&mut self, device_id: String, device: cpal::Device, config: cpal::StreamConfig, target_sample_rate: u32) -> Result<()> {
        // Create AudioInputStream with RTRB Producer/Consumer pair
        let mut input_stream = AudioInputStream::new(device_id.clone(), device.name().unwrap_or_default(), target_sample_rate)?;
        
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
        self.stream_manager.add_input_stream(device_id.clone(), device, config, producer, target_sample_rate)?;
        
        self.metrics.input_streams = self.input_streams.len();
        Ok(())
    }

    fn handle_remove_input_stream(&mut self, device_id: String) -> bool {
        let removed = self.input_streams.remove(&device_id).is_some();
        self.stream_manager.remove_stream(&device_id);
        self.metrics.input_streams = self.input_streams.len();
        removed
    }

    async fn handle_add_output_stream(&mut self, device_id: String, device: cpal::Device, config: cpal::StreamConfig) -> Result<()> {
        let (output_stream, spmc_reader) = AudioOutputStream::new(device_id.clone(), device.name().unwrap_or_default(), config.sample_rate.0);
        
        self.output_streams.insert(device_id.clone(), output_stream);
        self.stream_manager.add_output_stream(device_id, device, config, spmc_reader)?;
        
        self.metrics.output_streams = self.output_streams.len();
        Ok(())
    }

    fn handle_update_effects(&mut self, device_id: String, effects: AudioEffectsChain) -> Result<()> {
        if let Some(input_stream) = self.input_streams.get_mut(&device_id) {
            if let Ok(mut effects_guard) = input_stream.effects_chain.try_lock() {
                *effects_guard = effects;
                Ok(())
            } else {
                Err(anyhow::anyhow!("Could not lock effects chain"))
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
                let rms = (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
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
    fn get_samples_for_device(&mut self, device_id: &str, channel_config: &crate::audio::types::AudioChannel) -> Vec<f32> {
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
        }
    }

    /// Add input stream with RTRB Producer for lock-free audio capture
    pub fn add_input_stream(
        &mut self,
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        producer: Producer<f32>, // Owned RTRB Producer (not Arc - avoids Send+Sync issues)
        target_sample_rate: u32,
    ) -> Result<()> {
        use cpal::traits::{DeviceTrait, StreamTrait};
        use cpal::SampleFormat;
        
        // Get device's default format to determine sample format
        let supported_config = device.default_input_config().map_err(|e| anyhow::anyhow!("Failed to get default input config: {}", e))?;
        let sample_format = supported_config.sample_format();
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate.0;
        
        info!("🎤 Creating input stream for device '{}' with config: {}Hz, {} channels, format: {:?}", 
            device_id, sample_rate, channels, sample_format);
        
        // Move producer into the callback closure (owned, not shared)
        let mut producer = producer;
        let device_id_for_f32_callback = device_id.clone(); 
        let device_id_for_f32_error = device_id.clone();
        let device_id_for_i16_callback = device_id.clone();
        let device_id_for_i16_error = device_id.clone();
        
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
                        
                        // Debug logging for audio capture
                        use std::sync::{LazyLock, Mutex as StdMutex};
                        static CAPTURE_COUNT: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
                            LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

                        if let Ok(mut count_map) = CAPTURE_COUNT.lock() {
                            let count = count_map.entry(device_id_for_f32_callback.clone()).or_insert(0);
                            *count += 1;

                            if *count % 100 == 0 || (*count < 10) {
                                let peak = data.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                                let rms = (data.iter().map(|&s| s * s).sum::<f32>() / data.len() as f32).sqrt();
                                println!("🎵 RTRB_CAPTURE [{}]: Captured {} samples (call #{}), wrote: {}, dropped: {}, peak: {:.4}, rms: {:.4}",
                                    device_id_for_f32_callback, data.len(), count, samples_written, samples_dropped, peak, rms);
                            }
                        }
                    },
                    move |err| {
                        error!("❌ Input stream error for device '{}': {}", device_id_for_f32_error, err);
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
                        
                        // Debug logging for audio capture
                        use std::sync::{LazyLock, Mutex as StdMutex};
                        static CAPTURE_COUNT_I16: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
                            LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

                        if let Ok(mut count_map) = CAPTURE_COUNT_I16.lock() {
                            let count = count_map.entry(device_id_for_i16_callback.clone()).or_insert(0);
                            *count += 1;

                            if *count % 100 == 0 || (*count < 10) {
                                let peak = data.iter().map(|&s| (s as f32 / 32768.0).abs()).fold(0.0f32, f32::max);
                                println!("🎵 RTRB_CAPTURE_I16 [{}]: Captured {} samples (call #{}), wrote: {}, dropped: {}, peak: {:.4}",
                                    device_id_for_i16_callback, data.len(), count, samples_written, samples_dropped, peak);
                            }
                        }
                    },
                    move |err| {
                        error!("❌ Input stream error for device '{}': {}", device_id_for_i16_error, err);
                    },
                    None, // Timeout - use default
                )?
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported sample format: {:?}", sample_format));
            }
        };
        
        // Start the stream
        stream.play()?;
        
        // Store the stream
        self.streams.insert(device_id.clone(), stream);
        
        info!("✅ Input stream created and started for device '{}'", device_id);
        Ok(())
    }
    
    // LEGACY FUNCTIONS DISABLED: These contained RTRB Send+Sync issues - replaced by command channel

    pub fn add_input_stream_with_error_handling(
        &mut self,
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        audio_buffer: Arc<Producer<f32>>, // RTRB Producer for audio callback (lock-free!)
        target_sample_rate: u32,
        device_manager: Option<std::sync::Weak<crate::audio::devices::AudioDeviceManager>>,
    ) -> Result<()> {
        // STUB: Legacy function disabled due to RTRB Send+Sync issues
        println!("STUB: add_input_stream_with_error_handling called for device: {}", device_id);
        Ok(())
    }
    
    #[cfg(feature = "disabled")]
    // ALL LEGACY FUNCTIONS WITH RTRB CALLBACKS DISABLED DUE TO Send+Sync ISSUES
    pub fn legacy_functions_disabled() {
        // This section contains all the legacy functions with RTRB Send+Sync issues
        // They are disabled until the command channel architecture is fully implemented
    }
    
    /// Remove a stream by device ID  
    pub fn remove_stream(&mut self, device_id: &str) -> bool {
        if let Some(stream) = self.streams.remove(device_id) {
            println!("Stopping and removing stream for device: {}", device_id);
            drop(stream);
            true
        } else {
            println!("Stream not found for removal: {}", device_id);
            false
        }
    }

    /// Add output stream with SPMC Reader for lock-free audio playback
    pub fn add_output_stream(
        &mut self,
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        mut spmc_reader: spmcq::Reader<f32>,
    ) -> Result<()> {
        use cpal::traits::{DeviceTrait, StreamTrait};
        use cpal::SampleFormat;
        
        // Get device's default format to determine sample format
        let supported_config = device.default_output_config().map_err(|e| anyhow::anyhow!("Failed to get default output config: {}", e))?;
        let sample_format = supported_config.sample_format();
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate.0;
        
        info!("🔊 Creating output stream for device '{}' with config: {}Hz, {} channels, format: {:?}", 
            device_id, sample_rate, channels, sample_format);
        
        let device_id_for_f32_out_callback = device_id.clone();
        let device_id_for_f32_out_error = device_id.clone(); 
        let device_id_for_i16_out_callback = device_id.clone();
        let device_id_for_i16_out_error = device_id.clone();
        
        // Create the output stream with appropriate callback based on sample format
        let stream = match sample_format {
            SampleFormat::F32 => {
                device.build_output_stream(
                    &config,
                    move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                        // SPMC: Lock-free audio playback directly from hardware callback
                        let mut samples_read = 0;
                        let mut silence_filled = 0;
                        
                        // Fill the output buffer from SPMC queue
                        for output_sample in data.iter_mut() {
                            match spmc_reader.read() {
                                spmcq::ReadResult::Ok(audio_sample) => {
                                    *output_sample = audio_sample;
                                    samples_read += 1;
                                }
                                spmcq::ReadResult::Dropout(audio_sample) => {
                                    // Got data but missed some samples, still use the audio
                                    *output_sample = audio_sample;
                                    samples_read += 1;
                                }
                                spmcq::ReadResult::Empty => {
                                    // No data available, fill with silence
                                    *output_sample = 0.0;
                                    silence_filled += 1;
                                }
                            }
                        }
                        
                        // Debug logging for audio playback
                        use std::sync::{LazyLock, Mutex as StdMutex};
                        static PLAYBACK_COUNT: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
                            LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

                        if let Ok(mut count_map) = PLAYBACK_COUNT.lock() {
                            let count = count_map.entry(device_id_for_f32_out_callback.clone()).or_insert(0);
                            *count += 1;

                            if *count % 100 == 0 || (*count < 10) {
                                let peak = data.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                                let rms = (data.iter().map(|&s| s * s).sum::<f32>() / data.len() as f32).sqrt();
                                println!("🎵 SPMC_PLAYBACK [{}]: Playing {} samples (call #{}), read: {}, silence: {}, peak: {:.4}, rms: {:.4}",
                                    device_id_for_f32_out_callback, data.len(), count, samples_read, silence_filled, peak, rms);
                            }
                        }
                    },
                    move |err| {
                        error!("❌ Output stream error for device '{}': {}", device_id_for_f32_out_error, err);
                    },
                    None, // Timeout - use default
                )?
            }
            SampleFormat::I16 => {
                device.build_output_stream(
                    &config,
                    move |data: &mut [i16], _info: &cpal::OutputCallbackInfo| {
                        // Read f32 samples from SPMC and convert to i16
                        let mut samples_read = 0;
                        let mut silence_filled = 0;
                        
                        for output_sample in data.iter_mut() {
                            match spmc_reader.read() {
                                spmcq::ReadResult::Ok(audio_sample) => {
                                    // Convert f32 to i16
                                    *output_sample = (audio_sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                                    samples_read += 1;
                                }
                                spmcq::ReadResult::Dropout(audio_sample) => {
                                    // Got data but missed some samples, still use the audio
                                    *output_sample = (audio_sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                                    samples_read += 1;
                                }
                                spmcq::ReadResult::Empty => {
                                    // No data available, fill with silence
                                    *output_sample = 0;
                                    silence_filled += 1;
                                }
                            }
                        }
                        
                        // Debug logging for i16 playback
                        use std::sync::{LazyLock, Mutex as StdMutex};
                        static PLAYBACK_COUNT_I16: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> =
                            LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));

                        if let Ok(mut count_map) = PLAYBACK_COUNT_I16.lock() {
                            let count = count_map.entry(device_id_for_i16_out_callback.clone()).or_insert(0);
                            *count += 1;

                            if *count % 100 == 0 || (*count < 10) {
                                let peak = data.iter().map(|&s| (s as f32 / 32767.0).abs()).fold(0.0f32, f32::max);
                                println!("🎵 SPMC_PLAYBACK_I16 [{}]: Playing {} samples (call #{}), read: {}, silence: {}, peak: {:.4}",
                                    device_id_for_i16_out_callback, data.len(), count, samples_read, silence_filled, peak);
                            }
                        }
                    },
                    move |err| {
                        error!("❌ Output stream error for device '{}': {}", device_id_for_i16_out_error, err);
                    },
                    None, // Timeout - use default
                )?
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported sample format: {:?}", sample_format));
            }
        };
        
        // Start the stream
        stream.play()?;
        
        // Store the stream
        self.streams.insert(device_id.clone(), stream);
        
        info!("✅ Output stream created and started for device '{}'", device_id);
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
