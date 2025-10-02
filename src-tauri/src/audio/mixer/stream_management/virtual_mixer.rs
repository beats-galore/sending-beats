// Stream operations for VirtualMixer
//
// This module contains all the VirtualMixer methods related to stream lifecycle
// management, including adding/removing input/output streams, device switching,
// and stream configuration operations.

use anyhow::{Context, Result};
use colored::*;
use std::collections::HashMap;
use std::sync::{atomic::Ordering, Arc, Mutex};
use tracing::{error, info, warn};

#[derive(Debug)]
pub struct VirtualMixer {}

impl VirtualMixer {
    /// Create a new virtual mixer with lock-free resampler architecture
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self {})
    }

    /// Apply automatic gain reduction to maintain target RMS level
    /// This is disabled by default and should be made configurable (see GitHub issue)
    fn apply_auto_gain_reduction(buffer: &mut [f32], active_channels: usize) {
        // Calculate RMS (Root Mean Square) to determine actual audio energy
        let sum_of_squares: f32 = buffer.iter().map(|&s| s * s).sum();
        let rms = (sum_of_squares / buffer.len() as f32).sqrt();

        // Only apply gain reduction if we actually have significant signal energy
        // Standard: -18dBFS RMS ≈ 0.125 linear scale
        const RMS_TARGET: f32 = 0.125;

        if rms > RMS_TARGET {
            // Apply gentle compression instead of hard division
            // Use a soft-knee approach that preserves quiet signals
            let compression_ratio = (RMS_TARGET / rms).sqrt(); // Soft compression
            let gain_reduction = compression_ratio.max(0.7); // Limit max reduction to 30%

            for sample in buffer.iter_mut() {
                *sample *= gain_reduction;
            }

            // Log when we apply significant gain reduction
            if gain_reduction < 0.9 {
                info!(
                    "🎛️ {}: Applied dynamic gain reduction {:.2} (RMS: {:.3}, channels: {})",
                    "AUTO_GAIN_REDUCTION".bright_yellow(),
                    gain_reduction,
                    rms,
                    active_channels
                );
            }
        }
    }

    ///  audio mixing utility with stereo processing, smart gain management, and level calculation
    /// This operates on samples that are already at the same sample rate (after convert_inputs_to_mix_rate)
    pub fn mix_input_samples_ref(input_samples: &[(String, &[f32])]) -> Vec<f32> {
        // TODO: Make automatic gain reduction configurable (see GitHub issue)
        // For now, disabled by default as it should be user-controlled
        const ENABLE_AUTO_GAIN_REDUCTION: bool = false;

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

            // **GAIN STAGING FIX**: First pass - accumulate samples and count active channels
            for (device_id, samples) in input_samples.iter() {
                if !samples.is_empty() {
                    active_channels += 1;

                    let stereo_samples = samples;

                    let mix_length = buffer.len().min(stereo_samples.len());

                    for i in 0..mix_length {
                        if i < buffer.len() && i < stereo_samples.len() {
                            buffer[i] += stereo_samples[i];
                        }
                    }
                }
            }

            if active_channels > 1 && ENABLE_AUTO_GAIN_REDUCTION {
                Self::apply_auto_gain_reduction(&mut buffer, active_channels);
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
                    "VIRTUAL_MIXER_SLOW".red(),
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
