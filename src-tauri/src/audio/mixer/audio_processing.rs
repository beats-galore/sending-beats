// Real-time audio processing and mixing logic
//
// This module handles the core audio processing functionality including
// buffer optimization, audio level calculations, and real-time mixing
// operations that occur in audio callback threads.

use anyhow::Result;
use cpal::{BufferSize, Device};
use cpal::traits::DeviceTrait;
use std::collections::HashMap;
use tracing::{info, warn};

use super::super::types::AudioMetrics;
use super::types::VirtualMixer;

/// Audio format conversion and buffer management utilities
pub struct AudioFormatConverter;

impl AudioFormatConverter {
    /// Optimized I16 to F32 conversion
    #[inline]
    pub fn convert_i16_to_f32_optimized(i16_samples: &[i16]) -> Vec<f32> {
        i16_samples.iter()
            .map(|&sample| {
                if sample >= 0 {
                    sample as f32 / 32767.0  // Positive: divide by 32767
                } else {
                    sample as f32 / 32768.0  // Negative: divide by 32768 
                }
            })
            .collect()
    }

    /// Optimized U16 to F32 conversion
    #[inline]
    pub fn convert_u16_to_f32_optimized(u16_samples: &[u16]) -> Vec<f32> {
        u16_samples.iter()
            .map(|&sample| (sample as f32 - 32768.0) / 32767.5)  // Better symmetry
            .collect()
    }

    /// Centralized buffer overflow management
    pub fn manage_buffer_overflow_optimized(
        buffer: &mut Vec<f32>, 
        target_sample_rate: u32, 
        device_id: &str, 
        callback_count: u64
    ) {
        let max_buffer_size = target_sample_rate as usize; // 1 second max buffer
        let overflow_threshold = max_buffer_size + (max_buffer_size / 4); // 1.25 seconds
        
        if buffer.len() > overflow_threshold {
            let target_size = max_buffer_size * 7 / 8; // Keep 87.5% of max buffer
            
            if buffer.len() > target_size {
                let crossfade_samples = 64; // Small crossfade to prevent clicks/pops
                let start_index = buffer.len() - target_size;
                
                // Apply crossfading only if we have enough samples
                if start_index >= crossfade_samples {
                    for i in 0..crossfade_samples {
                        let fade_out = 1.0 - (i as f32 / crossfade_samples as f32);
                        let fade_in = i as f32 / crossfade_samples as f32;
                        
                        let old_sample = buffer[start_index - crossfade_samples + i];
                        let new_sample = buffer[start_index + i];
                        buffer[start_index + i] = old_sample * fade_out + new_sample * fade_in;
                    }
                }
                
                // Remove the old portion
                let new_buffer = buffer.split_off(start_index);
                *buffer = new_buffer;
                
                if callback_count % 100 == 0 {
                    println!("🔧 BUFFER OPTIMIZATION [{}]: Kept latest {} samples, buffer now {} samples (max: {})", 
                        device_id, target_size, buffer.len(), max_buffer_size);
                }
            }
        }
    }
}

impl VirtualMixer {

    /// Get current audio metrics for monitoring
    pub async fn get_metrics(&self) -> AudioMetrics {
        let metrics = self.metrics.lock().await;
        metrics.clone()
    }

    /// Get channel audio levels for VU meters (cached version for UI)
    pub async fn get_channel_levels(&self) -> HashMap<u32, (f32, f32, f32, f32)> {
        // Use cached levels to avoid blocking UI on real-time data locks
        let cache = self.channel_levels_cache.lock().await;
        cache.clone()
    }

    /// Update channel audio levels from real-time processing
    pub async fn update_channel_levels(&self, channel_id: u32, peak_left: f32, rms_left: f32, peak_right: f32, rms_right: f32) -> Result<()> {
        // Validate audio levels
        super::validation::validate_audio_levels(peak_left, rms_left, peak_right, rms_right)?;
        
        // Update real-time levels
        {
            let mut levels = self.channel_levels.lock().await;
            levels.insert(channel_id, (peak_left, rms_left, peak_right, rms_right));
        }
        
        // Update cached levels for UI (less frequently to reduce lock contention)
        // This could be done on a timer rather than every update for better performance
        {
            let mut cache = self.channel_levels_cache.lock().await;
            cache.insert(channel_id, (peak_left, rms_left, peak_right, rms_right));
        }
        
        Ok(())
    }

    /// Get master audio levels for VU meters (cached version for UI)
    pub async fn get_master_levels(&self) -> (f32, f32, f32, f32) {
        // Use cached levels to avoid blocking UI on real-time data locks
        let cache = self.master_levels_cache.lock().await;
        *cache
    }

    /// Update master audio levels from real-time processing
    pub async fn update_master_levels(&self, peak_left: f32, rms_left: f32, peak_right: f32, rms_right: f32) -> Result<()> {
        // Validate audio levels
        super::validation::validate_audio_levels(peak_left, rms_left, peak_right, rms_right)?;
        
        // Update real-time levels
        {
            let mut levels = self.master_levels.lock().await;
            *levels = (peak_left, rms_left, peak_right, rms_right);
        }
        
        // Update cached levels for UI
        {
            let mut cache = self.master_levels_cache.lock().await;
            *cache = (peak_left, rms_left, peak_right, rms_right);
        }
        
        Ok(())
    }

    /// Process audio buffer and calculate levels (simplified version for modular structure)
    pub async fn process_audio_buffer(&self, buffer: &mut [f32]) -> Result<()> {
        let buffer_len = buffer.len();
        if buffer_len == 0 {
            return Ok(());
        }
        
        // Calculate stereo peak and RMS levels
        let (peak_left, peak_right, rms_left, rms_right) = if buffer_len >= 2 {
            AudioLevelCalculator::calculate_stereo_levels(buffer)
        } else {
            // Mono fallback
            let mono_peak = AudioLevelCalculator::calculate_peak_level(buffer);
            let mono_rms = AudioLevelCalculator::calculate_rms_level(buffer);
            (mono_peak, mono_peak, mono_rms, mono_rms)
        };
        
        // Update master levels
        self.update_master_levels(peak_left, rms_left, peak_right, rms_right).await?;
        
        // Update audio clock
        {
            let mut clock = self.audio_clock.lock().await;
            if let Some(timing_sync) = clock.update(buffer_len / 2) { // Assuming stereo
                // Update timing metrics
                let mut timing_metrics = self.timing_metrics.lock().await;
                timing_metrics.update(&timing_sync);
            }
        }
        
        // Update metrics
        {
            let mut metrics = self.metrics.lock().await;
            metrics.samples_processed += buffer_len as u64;
            metrics.last_process_time = std::time::Instant::now();
        }
        
        Ok(())
    }
}

/// Audio level calculation utilities
pub struct AudioLevelCalculator;

impl AudioLevelCalculator {
    /// Calculate peak level from audio buffer
    pub fn calculate_peak_level(buffer: &[f32]) -> f32 {
        buffer.iter().map(|&sample| sample.abs()).fold(0.0, f32::max)
    }
    
    /// Calculate RMS (root mean square) level from audio buffer
    pub fn calculate_rms_level(buffer: &[f32]) -> f32 {
        if buffer.is_empty() {
            return 0.0;
        }
        
        let sum_squares: f32 = buffer.iter().map(|&sample| sample * sample).sum();
        (sum_squares / buffer.len() as f32).sqrt()
    }
    
    /// Calculate stereo levels from interleaved stereo buffer
    pub fn calculate_stereo_levels(buffer: &[f32]) -> (f32, f32, f32, f32) {
        if buffer.len() < 2 {
            return (0.0, 0.0, 0.0, 0.0);
        }
        
        let mut peak_left = 0.0f32;
        let mut peak_right = 0.0f32;
        let mut sum_squares_left = 0.0f32;
        let mut sum_squares_right = 0.0f32;
        let mut sample_count = 0;
        
        // Process interleaved stereo samples
        for chunk in buffer.chunks_exact(2) {
            let left = chunk[0].abs();
            let right = chunk[1].abs();
            
            peak_left = peak_left.max(left);
            peak_right = peak_right.max(right);
            
            sum_squares_left += chunk[0] * chunk[0];
            sum_squares_right += chunk[1] * chunk[1];
            
            sample_count += 1;
        }
        
        // Calculate RMS levels
        let rms_left = if sample_count > 0 {
            (sum_squares_left / sample_count as f32).sqrt()
        } else {
            0.0
        };
        
        let rms_right = if sample_count > 0 {
            (sum_squares_right / sample_count as f32).sqrt()
        } else {
            0.0
        };
        
        (peak_left, peak_right, rms_left, rms_right)
    }
    
    /// Convert linear level to decibels
    pub fn linear_to_db(linear: f32) -> f32 {
        if linear <= 0.0 {
            -std::f32::INFINITY
        } else {
            20.0 * linear.log10()
        }
    }
    
    /// Convert decibels to linear level
    pub fn db_to_linear(db: f32) -> f32 {
        if db.is_finite() {
            10.0_f32.powf(db / 20.0)
        } else {
            0.0
        }
    }
    
    /// Apply basic mixing to combine multiple audio channels
    pub fn mix_channels(channels: &[&[f32]], output: &mut [f32]) {
        // Clear output buffer
        output.fill(0.0);
        
        if channels.is_empty() {
            return;
        }
        
        // Mix all channels together
        for channel_buffer in channels {
            let len = output.len().min(channel_buffer.len());
            for i in 0..len {
                output[i] += channel_buffer[i];
            }
        }
        
        // Apply soft limiting to prevent clipping
        let max_level = output.iter().map(|&sample| sample.abs()).fold(0.0, f32::max);
        if max_level > 1.0 {
            let gain_reduction = 0.95 / max_level; // Leave small headroom
            for sample in output.iter_mut() {
                *sample *= gain_reduction;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peak_level_calculation() {
        let buffer = [0.5, -0.8, 0.3, -0.1];
        let peak = AudioLevelCalculator::calculate_peak_level(&buffer);
        assert!((peak - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_rms_level_calculation() {
        let buffer = [1.0, 0.0, -1.0, 0.0];
        let rms = AudioLevelCalculator::calculate_rms_level(&buffer);
        let expected = (2.0 / 4.0).sqrt(); // sqrt(0.5)
        assert!((rms - expected).abs() < 0.001);
    }

    #[test]
    fn test_stereo_levels() {
        let buffer = [0.5, -0.3, -0.8, 0.6]; // Left: 0.5, -0.8; Right: -0.3, 0.6
        let (peak_left, peak_right, rms_left, rms_right) = 
            AudioLevelCalculator::calculate_stereo_levels(&buffer);
            
        assert!((peak_left - 0.8).abs() < f32::EPSILON);
        assert!((peak_right - 0.6).abs() < f32::EPSILON);
        assert!(rms_left > 0.0);
        assert!(rms_right > 0.0);
    }

    #[test]
    fn test_db_conversion() {
        let linear = 0.5;
        let db = AudioLevelCalculator::linear_to_db(linear);
        let back_to_linear = AudioLevelCalculator::db_to_linear(db);
        assert!((linear - back_to_linear).abs() < 0.001);
    }

    #[test]
    fn test_channel_mixing() {
        let channel1 = [0.5, 0.3];
        let channel2 = [0.2, -0.1];
        let channels = [&channel1[..], &channel2[..]];
        let mut output = [0.0, 0.0];
        
        AudioLevelCalculator::mix_channels(&channels, &mut output);
        
        assert!((output[0] - 0.7).abs() < f32::EPSILON);
        assert!((output[1] - 0.2).abs() < f32::EPSILON);
    }
}