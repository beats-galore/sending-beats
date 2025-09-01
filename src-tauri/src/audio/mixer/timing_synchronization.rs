// Audio clock synchronization and timing management
//
// This module provides precise audio timing synchronization, hardware callback
// coordination, and drift compensation for professional audio processing.
// It ensures sample-accurate synchronization between input and output streams.

use tracing::{info, warn};

/// **PRIORITY 5: Audio Clock Synchronization**
/// Master audio clock for timing synchronization between input and output streams
#[derive(Debug)]
pub struct AudioClock {
    sample_rate: u32,
    samples_processed: u64,
    start_time: std::time::Instant,
    last_sync_time: std::time::Instant,
    drift_compensation: f64, // Microseconds of drift compensation
    sync_interval_samples: u64, // Sync every N samples
    log_counter: u64, // Counter for reduced logging frequency
}

impl AudioClock {
    /// Create a new audio clock with specified sample rate and initial buffer size
    /// Buffer size will be updated when actual hardware streams are created
    pub fn new(sample_rate: u32, initial_buffer_size: u32) -> Self {
        let now = std::time::Instant::now();
        Self {
            sample_rate,
            samples_processed: 0,
            start_time: now,
            last_sync_time: now,
            drift_compensation: 0.0,
            sync_interval_samples: initial_buffer_size as u64, // Will be updated with hardware buffer size
            log_counter: 0,
        }
    }
    
    /// Update the sync interval to match actual hardware buffer size
    /// This is called when streams are created with known hardware buffer sizes
    pub fn set_hardware_buffer_size(&mut self, hardware_buffer_size: u32) {
        let old_interval = self.sync_interval_samples;
        self.sync_interval_samples = hardware_buffer_size as u64;
        if old_interval != self.sync_interval_samples {
            info!("🔄 BUFFER SIZE UPDATE: AudioClock sync interval updated from {} to {} samples", 
                  old_interval, self.sync_interval_samples);
        }
    }
    
    /// Update the clock with processed samples - now tracks hardware callback timing instead of software timing
    pub fn update(&mut self, samples_added: usize) -> Option<TimingSync> {
        self.samples_processed += samples_added as u64;
        
        // Check if it's time to sync (every sync_interval_samples)
        if self.samples_processed % self.sync_interval_samples == 0 {
            let now = std::time::Instant::now();
            
            // **CRITICAL FIX**: In callback-driven processing, we don't calculate "expected" timing
            // because the samples arrive exactly when the hardware provides them.
            // Instead, we only track callback consistency and hardware timing variations.
            
            let callback_interval_us = now.duration_since(self.last_sync_time).as_micros() as f64;
            let expected_interval_us = (self.sync_interval_samples as f64 * 1_000_000.0) / self.sample_rate as f64;
            
            // Only report drift if callback intervals are inconsistent with expected buffer timing
            // Allow 10% variation for hardware jitter - this is normal and expected
            let variation_threshold = expected_interval_us * 0.10;
            let timing_variation = (callback_interval_us - expected_interval_us).abs();
            
            let sync_info = TimingSync {
                samples_processed: self.samples_processed,
                callback_interval_us,
                expected_interval_us,
                timing_variation,
                is_drift_significant: timing_variation > variation_threshold,
            };
            
            // Only log significant variations (>10% from expected) - but reduce frequency dramatically
            if sync_info.is_drift_significant {
                self.log_counter += 1;
                // Log only every 1000th occurrence to reduce spam
                if self.log_counter % 1000 == 0 {
                    warn!(
                        "⏰ TIMING VARIATION (#{} occurrences): Callback interval {:.1}μs vs expected {:.1}μs (variation: {:.1}μs, {:.1}%)",
                        self.log_counter,
                        callback_interval_us,
                        expected_interval_us,
                        timing_variation,
                        (timing_variation / expected_interval_us) * 100.0
                    );
                }
            }
            
            self.last_sync_time = now;
            return Some(sync_info);
        }
        
        None
    }
    
    /// Get current playback position in samples
    pub fn get_samples_processed(&self) -> u64 {
        self.samples_processed
    }
    
    /// Get current playback position in seconds
    pub fn get_playback_time_seconds(&self) -> f64 {
        self.samples_processed as f64 / self.sample_rate as f64
    }
    
    /// Get elapsed real time since clock start
    pub fn get_elapsed_time(&self) -> std::time::Duration {
        std::time::Instant::now().duration_since(self.start_time)
    }
    
    /// Calculate timing drift between audio time and real time
    pub fn get_timing_drift_ms(&self) -> f64 {
        let audio_time_ms = self.get_playback_time_seconds() * 1000.0;
        let real_time_ms = self.get_elapsed_time().as_millis() as f64;
        audio_time_ms - real_time_ms
    }
    
    /// Reset the clock (typically called when stopping/starting)
    pub fn reset(&mut self) {
        let now = std::time::Instant::now();
        self.samples_processed = 0;
        self.start_time = now;
        self.last_sync_time = now;
        self.drift_compensation = 0.0;
        self.log_counter = 0;
        info!("🔄 CLOCK RESET: Audio clock reset to zero");
    }
    
    /// Get sample rate
    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }
    
    /// Update sample rate (for dynamic reconfiguration)
    pub fn set_sample_rate(&mut self, new_sample_rate: u32) {
        if new_sample_rate != self.sample_rate {
            info!("🔄 SAMPLE RATE CHANGE: {} Hz -> {} Hz", self.sample_rate, new_sample_rate);
            self.sample_rate = new_sample_rate;
            // Recalculate sync interval for new sample rate if needed
        }
    }
}

/// Timing synchronization information returned by clock updates
#[derive(Debug, Clone)]
pub struct TimingSync {
    pub samples_processed: u64,
    pub callback_interval_us: f64,
    pub expected_interval_us: f64,
    pub timing_variation: f64,
    pub is_drift_significant: bool,
}

impl TimingSync {
    /// Get timing variation as a percentage
    pub fn get_variation_percentage(&self) -> f64 {
        if self.expected_interval_us > 0.0 {
            (self.timing_variation / self.expected_interval_us) * 100.0
        } else {
            0.0
        }
    }
    
    /// Check if timing is within acceptable bounds
    pub fn is_timing_acceptable(&self) -> bool {
        !self.is_drift_significant
    }
}

/// Performance metrics for timing analysis
#[derive(Debug, Clone)]
pub struct TimingMetrics {
    pub total_callbacks: u64,
    pub total_samples_processed: u64,
    pub significant_variations: u64,
    pub max_variation_us: f64,
    pub average_callback_interval_us: f64,
    pub last_update: std::time::Instant,
}

impl TimingMetrics {
    /// Create new timing metrics
    pub fn new() -> Self {
        Self {
            total_callbacks: 0,
            total_samples_processed: 0,
            significant_variations: 0,
            max_variation_us: 0.0,
            average_callback_interval_us: 0.0,
            last_update: std::time::Instant::now(),
        }
    }
    
    /// Update metrics with new timing sync information
    pub fn update(&mut self, sync: &TimingSync) {
        self.total_callbacks += 1;
        self.total_samples_processed = sync.samples_processed;
        
        if sync.is_drift_significant {
            self.significant_variations += 1;
        }
        
        if sync.timing_variation > self.max_variation_us {
            self.max_variation_us = sync.timing_variation;
        }
        
        // Update running average of callback intervals
        let alpha = 0.1; // Exponential moving average factor
        if self.average_callback_interval_us == 0.0 {
            self.average_callback_interval_us = sync.callback_interval_us;
        } else {
            self.average_callback_interval_us = 
                (1.0 - alpha) * self.average_callback_interval_us + 
                alpha * sync.callback_interval_us;
        }
        
        self.last_update = std::time::Instant::now();
    }
    
    /// Get the percentage of callbacks with significant timing variations
    pub fn get_variation_percentage(&self) -> f64 {
        if self.total_callbacks > 0 {
            (self.significant_variations as f64 / self.total_callbacks as f64) * 100.0
        } else {
            0.0
        }
    }
    
    /// Check if timing performance is acceptable
    pub fn is_performance_acceptable(&self) -> bool {
        // Consider performance acceptable if less than 5% of callbacks have significant variations
        self.get_variation_percentage() < 5.0
    }
    
    /// Reset metrics (typically called when restarting audio processing)
    pub fn reset(&mut self) {
        *self = Self::new();
    }
    
    /// Get human-readable performance summary
    pub fn get_performance_summary(&self) -> String {
        format!(
            "Callbacks: {}, Variations: {:.1}%, Max variation: {:.1}μs, Avg interval: {:.1}μs", 
            self.total_callbacks,
            self.get_variation_percentage(),
            self.max_variation_us,
            self.average_callback_interval_us
        )
    }
}

impl Default for TimingMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_audio_clock_creation() {
        let clock = AudioClock::new(48000, 512);
        assert_eq!(clock.get_sample_rate(), 48000);
        assert_eq!(clock.get_samples_processed(), 0);
        assert_eq!(clock.get_playback_time_seconds(), 0.0);
    }

    #[test]
    fn test_clock_update() {
        let mut clock = AudioClock::new(48000, 512);
        
        // First update - shouldn't trigger sync yet
        let sync = clock.update(256);
        assert!(sync.is_none());
        
        // Second update - should trigger sync
        let sync = clock.update(256);
        assert!(sync.is_some());
        
        assert_eq!(clock.get_samples_processed(), 512);
    }

    #[test]
    fn test_timing_metrics() {
        let mut metrics = TimingMetrics::new();
        assert_eq!(metrics.get_variation_percentage(), 0.0);
        assert!(metrics.is_performance_acceptable());
        
        // Simulate timing sync with significant variation
        let sync = TimingSync {
            samples_processed: 512,
            callback_interval_us: 15000.0,
            expected_interval_us: 10000.0,
            timing_variation: 5000.0,
            is_drift_significant: true,
        };
        
        metrics.update(&sync);
        assert_eq!(metrics.total_callbacks, 1);
        assert_eq!(metrics.significant_variations, 1);
        assert_eq!(metrics.get_variation_percentage(), 100.0);
    }

    #[test]
    fn test_clock_reset() {
        let mut clock = AudioClock::new(48000, 512);
        clock.update(512);
        assert_eq!(clock.get_samples_processed(), 512);
        
        clock.reset();
        assert_eq!(clock.get_samples_processed(), 0);
        assert_eq!(clock.get_playback_time_seconds(), 0.0);
    }
}