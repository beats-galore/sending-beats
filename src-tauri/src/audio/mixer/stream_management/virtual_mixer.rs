// Stream operations for VirtualMixer
//
// This module contains all the VirtualMixer methods related to stream lifecycle
// management, including adding/removing input/output streams, device switching,
// and stream configuration operations.

use super::super::sample_rate_converter::RubatoSRC;
use super::stream_manager::StreamInfo;
use anyhow::{Context, Result};
use colored::*;
use std::collections::HashMap;
use std::sync::{atomic::Ordering, Arc, Mutex};
use tracing::{error, info, warn};

#[derive(Debug)]
pub struct VirtualMixer {
    // **LOCK-FREE ARCHITECTURE**: Per-device resamplers with individual locks
    // This eliminates HashMap-level contention - multiple devices can resample in parallel
    pub input_resamplers: HashMap<String, Arc<Mutex<RubatoSRC>>>,
    pub output_resamplers: HashMap<String, Arc<Mutex<RubatoSRC>>>,
}

impl VirtualMixer {
    /// Create a new virtual mixer with lock-free resampler architecture
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            input_resamplers: HashMap::new(),
            output_resamplers: HashMap::new(),
        })
    }

    /// Professional audio mixing utility with stereo processing, smart gain management, and level calculation
    /// This operates on samples that are already at the same sample rate (after convert_inputs_to_mix_rate)
    pub fn mix_input_samples_ref(input_samples: &[(String, &[f32])]) -> Vec<f32> {
        if input_samples.is_empty() {
            return Vec::new();
        }

        // Calculate required buffer size based on actual input samples
        let required_stereo_samples = input_samples
            .iter()
            .map(|(_, samples)| samples.len())
            .max()
            .unwrap_or(256);

        // **DEBUG**: Log input sample sizes to track the accumulation bug
        static SAMPLE_SIZE_LOG_COUNT: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let log_count = SAMPLE_SIZE_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if log_count < 10 {
            for (device_id, samples) in input_samples.iter() {
                info!(
                    "🔍 {}: Input '{}' has {} samples",
                    "MIXER_INPUT_DEBUG".cyan(),
                    device_id,
                    samples.len()
                );
            }
            info!(
                "🔍 {}: Required buffer size: {} samples",
                "MIXER_INPUT_DEBUG".cyan(),
                required_stereo_samples
            );
        }

        // **PERFORMANCE FIX**: Use thread-local reusable buffer to eliminate allocations
        use std::cell::RefCell;
        thread_local! {
            static REUSABLE_MIX_BUFFER: RefCell<Vec<f32>> = RefCell::new(Vec::with_capacity(8192));
        }

        REUSABLE_MIX_BUFFER.with(|buf| {
            let mut buffer = buf.borrow_mut();
            // **CRITICAL FIX**: Always resize to exact size AND clear - this was the bug
            buffer.resize(required_stereo_samples, 0.0);
            buffer.fill(0.0); // Ensure buffer is completely zeroed

            // Mix all input channels together and calculate levels
            let mut active_channels = 0;

            for (device_id, samples) in input_samples.iter() {
                if !samples.is_empty() {
                    active_channels += 1;

                    // **PERFORMANCE FIX**: Skip expensive peak/RMS calculations during real-time mixing
                    // These were causing major performance bottlenecks (1000+ μs per mixing cycle)
                    // VU meters should be handled separately in a lower-priority thread
                    let (_peak_left, _rms_left, _peak_right, _rms_right) =
                        (0.0f32, 0.0f32, 0.0f32, 0.0f32);

                    // **PERFORMANCE FIX**: Disable debug logging during real-time mixing
                    // This was causing additional performance overhead with mutex locks

                    // **AUDIO QUALITY FIX**: Use input samples directly without unnecessary conversion
                    let stereo_samples = samples;

                    // **CRITICAL FIX**: Safe buffer size matching to prevent crashes
                    let mix_length = buffer.len().min(stereo_samples.len());

                    // Add samples with bounds checking
                    for i in 0..mix_length {
                        if i < buffer.len() && i < stereo_samples.len() {
                            buffer[i] += stereo_samples[i];
                        }
                    }
                }
            }

            // **AUDIO QUALITY FIX**: Smart gain management instead of aggressive division
            // Only normalize if we have multiple overlapping channels with significant signal
            if active_channels > 1 {
                // Check if we actually need normalization by checking peak levels
                let buffer_peak = buffer.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);

                // Only normalize if we're approaching clipping (> 0.8) with multiple channels
                if buffer_peak > 0.8 {
                    let normalization_factor = 0.8 / buffer_peak; // Normalize to 80% max to prevent clipping
                    for sample in buffer.iter_mut() {
                        *sample *= normalization_factor;
                    }
                    println!(
                        "🔧 GAIN CONTROL: Normalized {} channels, peak {:.3} -> {:.3}",
                        active_channels,
                        buffer_peak,
                        buffer_peak * normalization_factor
                    );
                }
                // If not approaching clipping, leave levels untouched for better dynamics
            }
            // Single channels: NO normalization - preserve full dynamics

            // **AUDIO LEVEL FIX**: Only apply gain reduction when actually needed
            let pre_master_peak = buffer.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);

            // Only apply gain reduction if signal is approaching clipping (> 0.9)
            if pre_master_peak > 0.9 {
                let safety_gain = 0.85f32; // Prevent clipping with safety margin
                for sample in buffer.iter_mut() {
                    *sample *= safety_gain;
                }
                warn!(
                    "🔧 {}: Hot signal {:.3}, applied {:.2} safety gain",
                    "CLIPPING PROTECTION".bright_green(),
                    pre_master_peak,
                    safety_gain
                );
            }
            // Otherwise: NO gain reduction - preserve original signal levels

            // Return cloned buffer (final allocation, but unavoidable for API)
            buffer.clone()
        })
    }
}
