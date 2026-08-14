// Queue state tracking for SPMC queues that don't expose occupancy data
use colored::*;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::info;

/// Queue state information
#[derive(Debug, Clone)]
pub struct QueueInfo {
    pub queue_id: String,
    pub capacity: usize,
    pub estimated_occupancy: usize,
    pub total_written: usize,
    pub total_read: usize,
    pub usage_percent: f32,
    pub available: usize,
    pub integral_error: f32,
    pub ratio: f32,
    pub target_fill: f32,
}

impl QueueInfo {
    pub fn new(queue_id: String, capacity: usize) -> Self {
        let target = capacity as f32 * 0.5; // aim for half-full
        Self {
            queue_id,
            capacity,
            estimated_occupancy: 0,
            total_written: 0,
            total_read: 0,
            usage_percent: 0.0,
            available: capacity,
            integral_error: 0.0,
            ratio: 1.0,
            target_fill: target,
        }
    }

    /// Update with new write operation
    fn on_samples_written(&mut self, count: usize) {
        self.total_written += count;
        self.update_derived_fields();
    }

    /// Update with new read operation
    fn on_samples_read(&mut self, count: usize) {
        self.total_read += count;
        self.update_derived_fields();
    }

    /// Calculate derived fields from write/read counters
    fn update_derived_fields(&mut self) {
        // Estimate occupancy as difference between written and read
        // This can temporarily go negative if reads are reported before writes
        let occupancy_signed = self.total_written as i64 - self.total_read as i64;
        self.estimated_occupancy = occupancy_signed.max(0) as usize;

        // Clamp to capacity (queue can't hold more than capacity)
        self.estimated_occupancy = self.estimated_occupancy.min(self.capacity);

        // Calculate derived metrics
        self.usage_percent = (self.estimated_occupancy as f32 / self.capacity as f32) * 100.0;
        self.available = self.capacity.saturating_sub(self.estimated_occupancy);
    }
}

/// Thread-safe queue state tracker using atomic counters
/// Alternative approach for real-time contexts that can't use async commands
#[derive(Clone)]
pub struct AtomicQueueTracker {
    pub queue_id: String,
    pub capacity: usize,
    pub current_occupancy: Arc<AtomicUsize>,

    // PI control state (using atomics for interior mutability)
    target_fill: f32,
    integral_error: Arc<AtomicU32>, // Store as f32 bits
    last_ratio: Arc<AtomicU32>,     // Store as f32 bits

    // tuning parameters
    kp: f32,
    ki: f32,
    max_ratio_adjust: f32,
}

impl AtomicQueueTracker {
    pub fn new(queue_id: String, capacity: usize) -> Self {
        let target = capacity as f32 * 0.5; // aim for half-full
        info!(
            "{}: creating queue tracker for device {}, capacity {}",
            "ATOMIC_QUEUE_TRACKER".on_purple().white(),
            queue_id,
            capacity
        );
        Self {
            queue_id,
            capacity,
            current_occupancy: Arc::new(AtomicUsize::new(0)),
            target_fill: target,
            integral_error: Arc::new(AtomicU32::new(0.0f32.to_bits())), // Store 0.0 as bits
            last_ratio: Arc::new(AtomicU32::new(1.0f32.to_bits())),     // Store 1.0 as bits
            kp: 0.0005,                                                 // proportional gain (tune!)
            ki: 0.000001,                                               // integral gain (tune!)
            max_ratio_adjust: 0.01, // max ±1% ratio change per update
        }
    }

    /// Tell the drift controller where this queue is meant to sit
    ///
    /// Half of capacity is only right for a queue that is meant to run half
    /// full. These run at whatever the pacing keeps in them, which is about one
    /// buffer, against capacities many times that — so left at the default the
    /// controller reads a large deficit and resamples to fill the queue, putting
    /// back exactly the delay the pacing took out.
    pub fn with_target_fill(mut self, target_samples: usize) -> Self {
        self.target_fill = target_samples.max(1) as f32;
        self
    }

    /// Where the drift controller is steering this queue, in samples
    pub fn target_fill(&self) -> f32 {
        self.target_fill
    }

    /// Record samples written (called from producer thread) - ADD to queue occupancy, clamped to capacity
    pub fn record_samples_written(&self, count: usize) {
        let occupancy_before_add = self.current_occupancy.load(Ordering::Relaxed);

        // Calculate how much we can actually add without exceeding capacity
        let available_space = self.capacity.saturating_sub(occupancy_before_add);
        let samples_to_add = count.min(available_space);

        if samples_to_add > 0 {
            self.current_occupancy
                .fetch_add(samples_to_add, Ordering::Relaxed);
        }

        static WRITE_LOG: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let log_count = WRITE_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if log_count % 1000 == 0 {
            info!(
                "📝 {}: Device '{}' recording {} samples (write #{})",
                "SAMPLES_WRITTEN".on_purple().white(),
                self.queue_id,
                count,
                log_count
            );
        }
    }

    /// Record samples read (called from consumer thread) - SUBTRACT from queue occupancy, prevent underflow
    pub fn record_samples_read(&self, count: usize) {
        let occupancy_before_sub = self.current_occupancy.load(Ordering::Relaxed);

        // Prevent underflow - only subtract what's actually available
        let samples_to_subtract = count.min(occupancy_before_sub);
        if samples_to_subtract > 0 {
            self.current_occupancy
                .fetch_sub(samples_to_subtract, Ordering::Relaxed);
        }
    }

    /// Get current queue info (can be called from any thread)
    pub fn get_queue_info(&self) -> QueueInfo {
        let current_occupancy = self.current_occupancy.load(Ordering::Relaxed);

        // Clamp occupancy to capacity (can't exceed queue size)
        let estimated_occupancy = current_occupancy.min(self.capacity);

        let usage_percent = (estimated_occupancy as f32 / self.capacity as f32) * 100.0;
        let available = self.capacity.saturating_sub(estimated_occupancy);

        QueueInfo {
            queue_id: self.queue_id.clone(),
            capacity: self.capacity,
            estimated_occupancy,
            total_written: 0, // Removed to prevent overflow
            total_read: 0,    // Removed to prevent overflow
            usage_percent,
            available,
            integral_error: f32::from_bits(self.integral_error.load(Ordering::Relaxed)),
            ratio: f32::from_bits(self.last_ratio.load(Ordering::Relaxed)),
            target_fill: self.target_fill,
        }
    }

    pub fn adjust_ratio(&self, input_rate: u32, output_rate: u32) -> f32 {
        let current_occupancy = self.current_occupancy.load(Ordering::Relaxed);

        let target = self.target_fill;
        let error = current_occupancy as f32 - target;

        // Load current integral error as f32
        let current_integral_error = f32::from_bits(self.integral_error.load(Ordering::Relaxed));

        // PI control - update integral error
        let new_integral_error = current_integral_error + error;
        self.integral_error
            .store(new_integral_error.to_bits(), Ordering::Relaxed);

        let mut correction = self.kp * (error / target) + self.ki * (new_integral_error / target);

        // Clamp to max ±1%
        if correction > self.max_ratio_adjust {
            correction = self.max_ratio_adjust;
        } else if correction < -self.max_ratio_adjust {
            correction = -self.max_ratio_adjust;
        }

        let r_nom = output_rate as f32 / input_rate as f32;
        let r_eff = r_nom * (1.0 + correction);

        // Store new ratio
        self.last_ratio.store(r_eff.to_bits(), Ordering::Relaxed);
        r_eff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPACITY: usize = 8192;
    const OPERATING_LEVEL: usize = 256;

    fn tracker() -> AtomicQueueTracker {
        AtomicQueueTracker::new("test".to_string(), CAPACITY).with_target_fill(OPERATING_LEVEL)
    }

    #[test]
    fn a_queue_at_its_operating_level_is_left_alone() {
        let tracker = tracker();
        tracker.record_samples_written(OPERATING_LEVEL);

        let ratio = tracker.adjust_ratio(48_000, 48_000);

        // Nominal, because there is nothing to correct
        assert!((ratio - 1.0).abs() < 0.0001, "ratio drifted to {}", ratio);
    }

    #[test]
    fn a_queue_running_ahead_is_slowed_and_one_falling_behind_is_sped_up() {
        let ahead = tracker();
        ahead.record_samples_written(OPERATING_LEVEL * 4);
        assert!(ahead.adjust_ratio(48_000, 48_000) > 1.0);

        let behind = tracker();
        behind.record_samples_written(OPERATING_LEVEL / 4);
        assert!(behind.adjust_ratio(48_000, 48_000) < 1.0);
    }

    #[test]
    fn the_operating_level_is_what_is_steered_to_not_half_the_ring() {
        // Half of capacity is far above where the pipeline paces these queues, so
        // steering to it would resample to fill the ring and put back the delay
        // the pacing removed.
        let tracker = tracker();
        tracker.record_samples_written(OPERATING_LEVEL);

        assert_eq!(tracker.target_fill(), OPERATING_LEVEL as f32);
        assert!(tracker.adjust_ratio(48_000, 48_000) <= 1.0001);
    }

    #[test]
    fn a_default_tracker_still_aims_at_half_capacity() {
        let tracker = AtomicQueueTracker::new("test".to_string(), CAPACITY);
        assert_eq!(tracker.target_fill(), (CAPACITY / 2) as f32);
    }
}
