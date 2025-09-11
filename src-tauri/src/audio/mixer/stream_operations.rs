// Stream operations for VirtualMixer
//
// This module contains all the VirtualMixer methods related to stream lifecycle
// management, including adding/removing input/output streams, device switching,
// and stream configuration operations.

use anyhow::{Context, Result};
use std::sync::{atomic::Ordering, Arc};
use tracing::{error, info, warn};

use super::stream_management::{AudioInputStream, AudioOutputStream, StreamInfo};
use super::types::VirtualMixer;

impl VirtualMixer {
    /// Professional audio mixing utility with stereo processing, smart gain management, and level calculation
    /// This is now a utility function that can be called by IsolatedAudioManager
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
                    let right_samples: Vec<f32> = samples.iter().skip(1).step_by(2).copied().collect();

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
