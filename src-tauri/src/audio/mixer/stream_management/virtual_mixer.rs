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

        // **DEBUG**: Log buffer operations for performance analysis
        static BUFFER_DEBUG_COUNT: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let debug_count = BUFFER_DEBUG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if debug_count < 20 || debug_count % 1000 == 0 {
            info!(
                "🔍 {}: Required: {} samples, inputs: {}",
                "MIXER_BUFFER_DEBUG".cyan(),
                required_stereo_samples,
                input_samples.len()
            );
        }

        // **PERFORMANCE FIX**: Use thread-local reusable buffer to eliminate allocations
        use std::cell::RefCell;
        thread_local! {
            static REUSABLE_MIX_BUFFER: RefCell<Vec<f32>> = RefCell::new(Vec::with_capacity(8192));
        }

        REUSABLE_MIX_BUFFER.with(|buf| {
            let mut buffer = buf.borrow_mut();
            let buffer_start = std::time::Instant::now();

            // **PERFORMANCE FIX**: Only resize if buffer is smaller, avoid unnecessary operations
            if buffer.len() < required_stereo_samples {
                buffer.resize(required_stereo_samples, 0.0);
            } else {
                // **PERFORMANCE FIX**: Only zero the portion we'll actually use
                buffer.truncate(required_stereo_samples);
            }

            // **PERFORMANCE FIX**: Always zero buffer for consistent mixing behavior
            let fill_start = std::time::Instant::now();
            buffer.fill(0.0);
            let fill_duration = fill_start.elapsed();

            let buffer_setup_duration = buffer_start.elapsed();

            // **DEBUG**: Log slow buffer operations
            if buffer_setup_duration.as_micros() > 500 {
                warn!(
                    "🐌 {}: Slow buffer setup: {}μs (size: {}, fill: {}μs)",
                    "BUFFER_SLOW".red(),
                    buffer_setup_duration.as_micros(),
                    buffer.len(),
                    fill_duration.as_micros()
                );
            }

            // Mix all input channels together
            let mixing_start = std::time::Instant::now();
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

            let mixing_loop_duration = mixing_start.elapsed();

            // **PERFORMANCE FIX**: Skip expensive peak calculations during real-time mixing
            // Only do gain management if we actually have multiple channels that could clip
            let clipping_start = std::time::Instant::now();
            if active_channels > 1 {
                // **PERFORMANCE FIX**: Use a faster max sample detection (early exit on first clip)
                let mut needs_limiting = false;
                for &sample in buffer.iter() {
                    if sample.abs() > 0.95 {
                        needs_limiting = true;
                        break;
                    }
                }

                if needs_limiting {
                    // Only calculate full peak when we actually need to limit
                    let buffer_peak = buffer.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let normalization_factor = 0.85 / buffer_peak; // Normalize to 85% max to prevent clipping
                    for sample in buffer.iter_mut() {
                        *sample *= normalization_factor;
                    }
                    warn!(
                        "🔧 {}: Hot multi-channel signal {:.3}, applied {:.2} limiting",
                        "CLIPPING PROTECTION".bright_green(),
                        buffer_peak,
                        normalization_factor
                    );
                }
            }
            // Single channels: NO normalization - preserve full dynamics
            // **PERFORMANCE FIX**: Skip master peak check unless we detect potential clipping

            let clipping_duration = clipping_start.elapsed();

            // **DEBUG**: Log slow VirtualMixer operations
            let total_mix_duration = buffer_start.elapsed();
            if total_mix_duration.as_micros() > 800 {
                warn!(
                    "🐌 {}: Slow mix operation: total {}μs (buffer: {}μs, mixing: {}μs, clipping: {}μs, samples: {})",
                    "VIRTUALMIXER_SLOW".red(),
                    total_mix_duration.as_micros(),
                    buffer_setup_duration.as_micros(),
                    mixing_loop_duration.as_micros(),
                    clipping_duration.as_micros(),
                    buffer.len()
                );
            }

            // Return cloned buffer (final allocation, but unavoidable for API)
            let clone_start = std::time::Instant::now();
            let result = buffer.clone();
            let clone_duration = clone_start.elapsed();

            if clone_duration.as_micros() > 200 {
                warn!(
                    "🐌 {}: Slow buffer clone: {}μs (size: {})",
                    "BUFFER_CLONE_SLOW".red(),
                    clone_duration.as_micros(),
                    buffer.len()
                );
            }

            result
        })
    }
}
