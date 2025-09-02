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
    drift_compensation: f64,    // Microseconds of drift compensation
    sync_interval_samples: u64, // Sync every N samples
    log_counter: u64,           // Counter for reduced logging frequency
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

    /// Get the current audio timestamp in samples
    pub fn get_sample_timestamp(&self) -> u64 {
        self.samples_processed
    }

    /// Get the current sync interval in samples
    pub fn get_sync_interval(&self) -> u64 {
        self.sync_interval_samples
    }

    /// Update the sync interval to match actual hardware buffer size
    /// This is called when streams are created with known hardware buffer sizes
    pub fn set_hardware_buffer_size(&mut self, hardware_buffer_size: u32) {
        let old_interval = self.sync_interval_samples;
        self.sync_interval_samples = hardware_buffer_size as u64;
        if old_interval != self.sync_interval_samples {
            info!(
                "🔄 BUFFER SIZE UPDATE: AudioClock sync interval updated from {} to {} samples",
                old_interval, self.sync_interval_samples
            );
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
            let expected_interval_us =
                (self.sync_interval_samples as f64 * 1_000_000.0) / self.sample_rate as f64;

            // Only report drift if callback intervals are inconsistent with expected buffer timing
            // This detects real hardware timing issues, not software processing timing
            let interval_variation = callback_interval_us - expected_interval_us;

            // Only consider significant variations in hardware callback timing as real drift
            let is_hardware_drift = interval_variation.abs() > expected_interval_us * 0.1; // 10% variation threshold

            // Reset drift compensation since we're now hardware-synchronized
            self.drift_compensation = if is_hardware_drift {
                interval_variation
            } else {
                0.0
            };

            let sync = TimingSync {
                samples_processed: self.samples_processed,
                callback_interval_us,
                expected_interval_us,
                timing_variation: interval_variation,
                is_drift_significant: is_hardware_drift,
            };

            // Only log actual hardware timing issues, not software processing timing
            if is_hardware_drift {
                crate::audio_debug!("⏰ HARDWARE TIMING: Callback interval variation: {:.2}ms (expected: {:.2}ms, actual: {:.2}ms)", 
                    interval_variation / 1000.0, expected_interval_us / 1000.0, callback_interval_us / 1000.0);
            }

            self.last_sync_time = now;
            Some(sync)
        } else {
            None
        }
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

    /// Get the current drift compensation
    pub fn get_drift_compensation(&self) -> f64 {
        self.drift_compensation
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
            info!(
                "🔄 SAMPLE RATE CHANGE: {} Hz -> {} Hz",
                self.sample_rate, new_sample_rate
            );
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
    pub processing_time_avg_us: f64,
    pub processing_time_max_us: f64,
    pub buffer_underruns: u64,
    pub buffer_overruns: u64,
    pub sync_adjustments: u64,
    pub last_reset: std::time::Instant,
    sample_count: u64,
    processing_time_sum_us: f64,
}
impl TimingMetrics {
    /// Create new timing metrics
    pub fn new() -> Self {
        Self {
            processing_time_avg_us: 0.0,
            processing_time_max_us: 0.0,
            buffer_underruns: 0,
            buffer_overruns: 0,
            sync_adjustments: 0,
            last_reset: std::time::Instant::now(),
            sample_count: 0,
            processing_time_sum_us: 0.0,
        }
    }

    /// Record sync adjustment applied
    pub fn record_sync_adjustment(&mut self) {
        self.sync_adjustments += 1;
    }

    /// Record processing time for a buffer
    pub fn record_processing_time(&mut self, duration_us: f64) {
        self.processing_time_sum_us += duration_us;
        self.sample_count += 1;

        // Update max
        if duration_us > self.processing_time_max_us {
            self.processing_time_max_us = duration_us;
        }

        // Update rolling average
        self.processing_time_avg_us = self.processing_time_sum_us / self.sample_count as f64;
    }

    /// Record buffer underrun (not enough samples available)
    pub fn record_underrun(&mut self) {
        self.buffer_underruns += 1;
    }

    /// Reset metrics (typically called when restarting audio processing)
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Get human-readable performance summary
    pub fn get_performance_summary(&self) -> String {
        let uptime_sec = self.last_reset.elapsed().as_secs_f64();
        format!(
            "Audio Metrics ({}s): Avg Processing: {:.1}μs, Max: {:.1}μs, Underruns: {}, Overruns: {}, Sync Adjustments: {}",
            uptime_sec.round(),
            self.processing_time_avg_us,
            self.processing_time_max_us,
            self.buffer_underruns,
            self.buffer_overruns,
            self.sync_adjustments
        )
    }
}

impl Default for TimingMetrics {
    fn default() -> Self {
        Self::new()
    }
}
