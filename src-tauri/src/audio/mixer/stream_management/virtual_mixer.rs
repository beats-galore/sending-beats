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
    // Audio processing with persistent sample rate converters (thread-safe)
    pub input_resamplers: Arc<Mutex<HashMap<String, RubatoSRC>>>,
    pub output_resamplers: Arc<Mutex<HashMap<String, RubatoSRC>>>,
}

impl VirtualMixer {
    /// Create a new virtual mixer with default device manager
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            input_resamplers: Arc::new(Mutex::new(HashMap::new())),
            output_resamplers: Arc::new(Mutex::new(HashMap::new())),
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
                // Get or create persistent resampler for this device
                let resampler_key = format!("{}_{}_to_{}", device_id, input_rate, target_mix_rate);

                // Check if resampler exists and get/create it
                let mut should_create_resampler = false;
                if let Ok(resamplers) = self.input_resamplers.try_lock() {
                    should_create_resampler = !resamplers.contains_key(&resampler_key);
                }

                if should_create_resampler {
                    println!(
                        "🔧 PERSISTENT_SRC: Creating new input resampler for {} ({} Hz -> {} Hz)",
                        device_id, input_rate, target_mix_rate
                    );

                    match RubatoSRC::new(input_rate as f32, target_mix_rate as f32) {
                        Ok(resampler) => {
                            if let Ok(mut resamplers) = self.input_resamplers.try_lock() {
                                resamplers.insert(resampler_key.clone(), resampler);
                            }
                        }
                        Err(e) => {
                            println!(
                                "❌ PERSISTENT_SRC: Failed to create resampler for {}: {}",
                                device_id, e
                            );
                            converted_samples.push((device_id, samples));
                            continue;
                        }
                    }
                }

                // Use persistent resampler
                if let Ok(mut resamplers) = self.input_resamplers.try_lock() {
                    if let Some(resampler) = resamplers.get_mut(&resampler_key) {
                        // Calculate expected output size
                        let expected_output_size = ((samples.len() as f32 * target_mix_rate as f32)
                            / input_rate as f32)
                            as usize;
                        let converted = resampler.convert(&samples, expected_output_size);

                        // Rate-limited logging
                        use std::sync::{LazyLock, Mutex as StdMutex};
                        static CONVERSION_COUNT: LazyLock<StdMutex<u64>> =
                            LazyLock::new(|| StdMutex::new(0));
                        if let Ok(mut count) = CONVERSION_COUNT.lock() {
                            *count += 1;
                            if *count <= 3 || *count % 1000 == 0 {
                                println!("🔄 PERSISTENT_SRC: {} converted {} samples -> {} samples (call #{})",
                                         device_id, samples.len(), converted.len(), count);
                            }
                        }

                        converted_samples.push((device_id, converted));
                    } else {
                        // Fallback if resampler not found
                        converted_samples.push((device_id, samples));
                    }
                } else {
                    // Fallback if lock failed
                    converted_samples.push((device_id, samples));
                }
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
            // Get or create persistent resampler for this output device
            let resampler_key = format!("output_{}_{}_to_{}", device_id, mix_rate, output_rate);

            // Check if resampler exists and get/create it
            let mut should_create_resampler = false;
            if let Ok(resamplers) = self.output_resamplers.try_lock() {
                should_create_resampler = !resamplers.contains_key(&resampler_key);
            }

            if should_create_resampler {
                println!("🔧 PERSISTENT_OUTPUT_SRC: Creating new output resampler for {} ({} Hz -> {} Hz)",
                         device_id, mix_rate, output_rate);

                match RubatoSRC::new(mix_rate as f32, output_rate as f32) {
                    Ok(resampler) => {
                        if let Ok(mut resamplers) = self.output_resamplers.try_lock() {
                            resamplers.insert(resampler_key.clone(), resampler);
                        }
                    }
                    Err(e) => {
                        println!("❌ PERSISTENT_OUTPUT_SRC: Failed to create output resampler for {}: {}", device_id, e);
                        return mixed_samples;
                    }
                }
            }

            // Use persistent resampler
            if let Ok(mut resamplers) = self.output_resamplers.try_lock() {
                if let Some(resampler) = resamplers.get_mut(&resampler_key) {
                    // Let resampler do its job properly - no padding or manipulation
                    let converted = resampler.convert(&mixed_samples, 0); // Let resampler decide output size

                    // Rate-limited logging
                    use std::sync::{LazyLock, Mutex as StdMutex};
                    static OUTPUT_CONVERSION_COUNT: LazyLock<StdMutex<u64>> =
                        LazyLock::new(|| StdMutex::new(0));
                    if let Ok(mut count) = OUTPUT_CONVERSION_COUNT.lock() {
                        *count += 1;
                        if *count <= 3 || *count % 1000 == 0 {
                            println!("🔄 PERSISTENT_OUTPUT_SRC: {} converted {} samples -> {} samples (call #{})",
                                     device_id, mixed_samples.len(), converted.len(), count);
                        }
                    }

                    converted
                } else {
                    // Fallback if resampler not found
                    mixed_samples
                }
            } else {
                // Fallback if lock failed
                mixed_samples
            }
        } else {
            // No conversion needed
            mixed_samples
        }
    }

    /// Professional audio mixing utility with stereo processing, smart gain management, and level calculation
    /// This operates on samples that are already at the same sample rate (after convert_inputs_to_mix_rate)
    pub fn mix_input_samples(input_samples: Vec<(String, Vec<f32>)>) -> Vec<f32> {
        if input_samples.is_empty() {
            return Vec::new();
        }

        // Calculate required buffer size based on actual input samples
        let required_stereo_samples = input_samples
            .iter()
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
                    let right_samples: Vec<f32> =
                        samples.iter().skip(1).step_by(2).copied().collect();

                    let peak_left = left_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let rms_left = if !left_samples.is_empty() {
                        (left_samples.iter().map(|&s| s * s).sum::<f32>()
                            / left_samples.len() as f32)
                            .sqrt()
                    } else {
                        0.0
                    };

                    let peak_right = right_samples
                        .iter()
                        .map(|&s| s.abs())
                        .fold(0.0f32, f32::max);
                    let rms_right = if !right_samples.is_empty() {
                        (right_samples.iter().map(|&s| s * s).sum::<f32>()
                            / right_samples.len() as f32)
                            .sqrt()
                    } else {
                        0.0
                    };

                    (peak_left, rms_left, peak_right, rms_right)
                } else {
                    // Mono audio: duplicate to both L/R channels
                    let peak_mono = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let rms_mono = if !samples.is_empty() {
                        (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
                    } else {
                        0.0
                    };

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
            let buffer_peak = reusable_output_buffer
                .iter()
                .map(|&s| s.abs())
                .fold(0.0f32, f32::max);

            // Only normalize if we're approaching clipping (> 0.8) with multiple channels
            if buffer_peak > 0.8 {
                let normalization_factor = 0.8 / buffer_peak; // Normalize to 80% max to prevent clipping
                for sample in reusable_output_buffer.iter_mut() {
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
        let pre_master_peak = reusable_output_buffer
            .iter()
            .map(|&s| s.abs())
            .fold(0.0f32, f32::max);

        // Only apply gain reduction if signal is approaching clipping (> 0.9)
        if pre_master_peak > 0.9 {
            let safety_gain = 0.85f32; // Prevent clipping with safety margin
            for sample in reusable_output_buffer.iter_mut() {
                *sample *= safety_gain;
            }
            println!(
                "🔧 CLIPPING PROTECTION: Hot signal {:.3}, applied {:.2} safety gain",
                pre_master_peak, safety_gain
            );
        }
        // Otherwise: NO gain reduction - preserve original signal levels

        reusable_output_buffer
    }
}
