use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::audio::effects::{AudioEffectsChain, EQBand};
use crate::audio::types::AudioChannel;
use tokio::sync::{mpsc, oneshot, Mutex, Notify};

// Lock-free audio buffer imports
use rtrb::{Consumer, Producer, RingBuffer};
use spmcq::{ring_buffer, ReadResult, Reader, Writer};

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
