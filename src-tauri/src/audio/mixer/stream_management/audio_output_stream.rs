use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::{error, info, warn};

use super::super::sample_rate_converter::LinearSRC;
use super::super::types::VirtualMixer;
use crate::audio::effects::{AudioEffectsChain, EQBand};
use crate::audio::types::AudioChannel;
use tokio::sync::{mpsc, oneshot, Mutex, Notify};

// Lock-free audio buffer imports
use rtrb::{Consumer, Producer, RingBuffer};
use spmcq::{ring_buffer, ReadResult, Reader, Writer};

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
                        println!(
                            "📝 SPMC_WRITE [{}]: Writing {} samples to SPMC queue, peak: {:.4}",
                            count,
                            samples.len(),
                            peak
                        );
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
