// Stream operations for VirtualMixer
//
// This module contains all the VirtualMixer methods related to stream lifecycle
// management, including adding/removing input/output streams, device switching,
// and stream configuration operations.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::{atomic::Ordering, Arc, Mutex};
use tracing::{error, info, warn};

use super::super::sample_rate_converter::RubatoSRC;
use super::stream_manager::StreamInfo;

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

    /// Convert input samples to target mix rate using professional RubatoSRC resampling
    /// Call this BEFORE effects processing to ensure all inputs are at the same rate

    // TODO: call this per input not on all of them at once.
    pub fn convert_inputs_to_mix_rate(
        &mut self,
        input_samples: Vec<(String, Vec<f32>)>,
        input_sample_rates: Vec<(String, u32)>,
        target_mix_rate: u32,
    ) -> Vec<(String, Vec<f32>)> {
        let mut converted_samples = Vec::new();

        for (device_id, samples) in input_samples {
            // Find the sample rate for this device
            let input_rate = input_sample_rates
                .iter()
                .find(|(id, _)| id == &device_id)
                .map(|(_, rate)| *rate)
                .unwrap_or(target_mix_rate);

            // Check if conversion is needed
            if (input_rate as f32 - target_mix_rate as f32).abs() > 1.0 {
                // **LOCK-FREE**: Get or create resampler for this specific device (no HashMap lock needed)
                let resampler_key = format!("{}_{}_to_{}", device_id, input_rate, target_mix_rate);

                // Get or create resampler for this device (no HashMap lock needed)
                let device_resampler = if let Some(existing_resampler) =
                    self.input_resamplers.get(&resampler_key)
                {
                    existing_resampler.clone()
                } else {
                    // Create new resampler for this device
                    println!(
                        "🔧 LOCKFREE_INPUT_SRC: Creating resampler for {} ({} Hz -> {} Hz)",
                        device_id, input_rate, target_mix_rate
                    );

                    match RubatoSRC::new_low_artifact(input_rate as f32, target_mix_rate as f32) {
                        Ok(resampler) => {
                            let resampler_arc = Arc::new(Mutex::new(resampler));
                            self.input_resamplers
                                .insert(resampler_key.clone(), resampler_arc.clone());
                            resampler_arc
                        }
                        Err(e) => {
                            println!(
                                "❌ LOCKFREE_INPUT_SRC: Failed to create resampler for {}: {}",
                                device_id, e
                            );
                            converted_samples.push((device_id, samples));
                            continue;
                        }
                    }
                };

                // **SIMPLIFIED**: Process samples with device-specific resampler (individual lock only)
                let processing_start = std::time::Instant::now();
                let converted = match device_resampler.try_lock() {
                    Ok(mut resampler) => {
                        // **SIMPLIFIED INPUT PROCESSING**: No chunk logic needed - just convert and pass through
                        let converted_result = resampler.convert(&samples);
                        converted_result
                    }
                    Err(_) => {
                        // Device-specific resampler is busy (rare - only if same device used simultaneously)
                        static DEVICE_LOCK_FAILS: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let fail_count =
                            DEVICE_LOCK_FAILS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if fail_count <= 10 {
                            println!("🔒 LOCKFREE_INPUT_BUSY: Device {} resampler busy, using original samples (fail #{})", 
                                device_id, fail_count);
                        }
                        // Fallback: use original samples
                        samples.clone()
                    }
                };

                let processing_duration = processing_start.elapsed();
                // Log if individual resampler processing took too long
                if processing_duration.as_micros() > 200 {
                    static SLOW_INPUT_PROCESSING: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(0);
                    let count =
                        SLOW_INPUT_PROCESSING.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if count <= 10 {
                        println!(
                            "⏱️ LOCKFREE_INPUT_SLOW: Device {} resampling took {}μs (slow #{})",
                            device_id,
                            processing_duration.as_micros(),
                            count
                        );
                    }
                }

                // Rate-limited logging
                use std::sync::{LazyLock, Mutex as StdMutex};
                static CONVERSION_COUNT: LazyLock<StdMutex<u64>> =
                    LazyLock::new(|| StdMutex::new(0));
                if let Ok(mut count) = CONVERSION_COUNT.lock() {
                    *count += 1;
                    if *count <= 3 || *count % 1000 == 0 {
                        println!("🔄 LOCKFREE_INPUT_SRC: {} converted {} samples -> {} samples (call #{})",
                                 device_id, samples.len(), converted.len(), count);
                    }
                }

                converted_samples.push((device_id, converted));

                // Processing complete - resampler logic handled above
            } else {
                // No conversion needed
                converted_samples.push((device_id, samples));
            }
        }

        converted_samples
    }

    /// Convert mixed output to specific output device sample rate using professional RubatoSRC
    /// Call this AFTER mixing to prepare samples for each output destination
    pub fn convert_output_to_device_rate(
        &mut self,
        device_id: &str,
        mixed_samples: Vec<f32>,
        mix_rate: u32,
        output_rate: u32,
    ) -> Vec<f32> {
        // Rate-limited debug logging (only first 3 calls)
        use std::sync::{LazyLock, Mutex as StdMutex};
        static DEBUG_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
        let should_log = if let Ok(mut count) = DEBUG_COUNT.try_lock() {
            *count += 1;
            *count <= 3
        } else {
            false
        };

        // Check if conversion is needed
        let rate_diff = (mix_rate as f32 - output_rate as f32).abs();
        if should_log {
            println!(
                "🔍 OUTPUT_CONVERSION_CHECK: {} Hz -> {} Hz, diff: {:.1} Hz",
                mix_rate, output_rate, rate_diff
            );
        }

        if rate_diff > 1.0 {
            // **LOCK-FREE**: Get or create resampler for this specific output device (no HashMap lock needed)
            let resampler_key = format!("output_{}_{}_to_{}", device_id, mix_rate, output_rate);

            // Get or create resampler for this output device (no HashMap lock needed)
            let device_resampler =
                if let Some(existing_resampler) = self.output_resamplers.get(&resampler_key) {
                    existing_resampler.clone()
                } else {
                    // Create new resampler for this output device
                    println!(
                        "🔧 LOCKFREE_OUTPUT_SRC: Creating resampler for {} ({} Hz -> {} Hz)",
                        device_id, mix_rate, output_rate
                    );

                    match RubatoSRC::new_low_artifact(mix_rate as f32, output_rate as f32) {
                        Ok(resampler) => {
                            let resampler_arc = Arc::new(Mutex::new(resampler));
                            self.output_resamplers
                                .insert(resampler_key.clone(), resampler_arc.clone());
                            resampler_arc
                        }
                        Err(e) => {
                            println!(
                                "❌ LOCKFREE_OUTPUT_SRC: Failed to create resampler for {}: {}",
                                device_id, e
                            );
                            return mixed_samples;
                        }
                    }
                };

            // **OUTPUT PROCESSING WITH CHUNK LOGIC**: This is where we need accumulator logic for hardware
            let processing_start = std::time::Instant::now();
            let converted = match device_resampler.try_lock() {
                Ok(mut resampler) => {
                    // **HARDWARE CHUNK REQUIREMENTS**: Check accumulator for proper device output sizing
                    let target_samples = resampler.get_target_chunk_size() * 2; // Stereo
                    let accumulator_size = resampler.get_accumulator_size();

                    if accumulator_size >= target_samples {
                        // Accumulator has enough samples - drain for hardware output
                        static DRAIN_COUNT: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let count = DRAIN_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if count <= 10 || count % 100 == 0 {
                            println!("🚰 LOCKFREE_OUTPUT_DRAIN: Draining {} samples (target: {}), ready for hardware (#{}))",
                                     accumulator_size, target_samples, count);
                        }
                        // Return hardware-ready samples
                        resampler.drain_accumulator_only()
                    } else {
                        // Accumulator needs more samples - process normally
                        resampler.convert(&mixed_samples)
                    }
                }
                Err(_) => {
                    // Device-specific output resampler is busy (rare - only if same device used simultaneously)
                    static OUTPUT_DEVICE_LOCK_FAILS: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(0);
                    let fail_count =
                        OUTPUT_DEVICE_LOCK_FAILS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if fail_count <= 10 {
                        println!("🔒 LOCKFREE_OUTPUT_BUSY: Device {} resampler busy, using original samples (fail #{})", 
                            device_id, fail_count);
                    }
                    // Fallback: use original samples
                    mixed_samples.clone()
                }
            };

            let processing_duration = processing_start.elapsed();
            // Log if individual device resampler took too long
            if processing_duration.as_micros() > 300 {
                static SLOW_OUTPUT_PROCESSING: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let count =
                    SLOW_OUTPUT_PROCESSING.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if count <= 10 {
                    println!(
                        "⏱️ LOCKFREE_OUTPUT_SLOW: Device {} took {}μs (slow #{})",
                        device_id,
                        processing_duration.as_micros(),
                        count
                    );
                }
            }

            // Rate-limited logging
            use std::sync::{LazyLock, Mutex as StdMutex};
            static OUTPUT_CONVERSION_COUNT: LazyLock<StdMutex<u64>> =
                LazyLock::new(|| StdMutex::new(0));
            if let Ok(mut count) = OUTPUT_CONVERSION_COUNT.lock() {
                *count += 1;
                if *count <= 3 || *count % 1000 == 0 {
                    println!(
                        "🔄 LOCKFREE_OUTPUT_SRC: {} converted {} samples -> {} samples (call #{})",
                        device_id,
                        mixed_samples.len(),
                        converted.len(),
                        count
                    );
                }
            }

            converted

            // Processing complete - lock-free resampler logic handled above
        } else {
            // No conversion needed
            mixed_samples
        }
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

        // **PERFORMANCE FIX**: Use thread-local reusable buffer to eliminate allocations
        use std::cell::RefCell;
        thread_local! {
            static REUSABLE_MIX_BUFFER: RefCell<Vec<f32>> = RefCell::new(Vec::with_capacity(8192));
        }

        REUSABLE_MIX_BUFFER.with(|buf| {
            let mut buffer = buf.borrow_mut();
            // Resize buffer only if needed (avoids allocation in most cases)
            if buffer.len() != required_stereo_samples {
                buffer.resize(required_stereo_samples, 0.0);
            } else {
                // Just clear existing buffer (much faster than allocation)
                buffer.fill(0.0);
            }

            // Mix all input channels together and calculate levels
            let mut active_channels = 0;

        for (device_id, samples) in input_samples.iter() {
            if !samples.is_empty() {
                active_channels += 1;

                // **PERFORMANCE FIX**: Skip expensive peak/RMS calculations during real-time mixing
                // These were causing major performance bottlenecks (1000+ μs per mixing cycle)
                // VU meters should be handled separately in a lower-priority thread
                let (_peak_left, _rms_left, _peak_right, _rms_right) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);

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
            let buffer_peak = buffer
                .iter()
                .map(|&s| s.abs())
                .fold(0.0f32, f32::max);

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
        let pre_master_peak = buffer
            .iter()
            .map(|&s| s.abs())
            .fold(0.0f32, f32::max);

        // Only apply gain reduction if signal is approaching clipping (> 0.9)
        if pre_master_peak > 0.9 {
            let safety_gain = 0.85f32; // Prevent clipping with safety margin
            for sample in buffer.iter_mut() {
                *sample *= safety_gain;
            }
            println!(
                "🔧 CLIPPING PROTECTION: Hot signal {:.3}, applied {:.2} safety gain",
                pre_master_peak, safety_gain
            );
            }
            // Otherwise: NO gain reduction - preserve original signal levels

            // Return cloned buffer (final allocation, but unavoidable for API)
            buffer.clone()
        })
    }
}
