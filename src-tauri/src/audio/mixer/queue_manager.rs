// Queue state tracking for SPMC queues that don't expose occupancy data
use colored::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering, AtomicU32};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, trace};

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
        info!("{}: creating queue tracker for device {}, capacity {}", "ATOMIC_QUEUE_TRACKER".on_purple().white(), queue_id, capacity);
        Self {
            queue_id,
            capacity,
            current_occupancy: Arc::new(AtomicUsize::new(0)),
            target_fill: target,
            integral_error: Arc::new(AtomicU32::new(0.0f32.to_bits())), // Store 0.0 as bits
            last_ratio: Arc::new(AtomicU32::new(1.0f32.to_bits())),     // Store 1.0 as bits
            kp: 0.0005,       // proportional gain (tune!)
            ki: 0.000001,     // integral gain (tune!)
            max_ratio_adjust: 0.01, // max ±1% ratio change per update
        }
    }

    /// Record samples written (called from producer thread) - ADD to queue occupancy, clamped to capacity
    pub fn record_samples_written(&self, count: usize) {
        let occupancy_before_add = self.current_occupancy.load(Ordering::Relaxed);

        // Calculate how much we can actually add without exceeding capacity
        let available_space = self.capacity.saturating_sub(occupancy_before_add);
        let samples_to_add = count.min(available_space);

        if samples_to_add > 0 {
            self.current_occupancy.fetch_add(samples_to_add, Ordering::Relaxed);
        }
    }

    /// Record samples read (called from consumer thread) - SUBTRACT from queue occupancy, prevent underflow
    pub fn record_samples_read(&self, count: usize) {
        let occupancy_before_sub = self.current_occupancy.load(Ordering::Relaxed);

        // Prevent underflow - only subtract what's actually available
        let samples_to_subtract = count.min(occupancy_before_sub);
        if samples_to_subtract > 0 {
            self.current_occupancy.fetch_sub(samples_to_subtract, Ordering::Relaxed);
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
            target_fill: self.target_fill
        }
    }

    pub fn adjust_ratio(&self, input_rate: u32, output_rate: u32) -> f32 {
        let current_occupancy = self.current_occupancy.load(Ordering::Relaxed);

        let target = (self.capacity / 2) as f32;
        let error = current_occupancy as f32 - target;

        // Load current integral error as f32
        let current_integral_error = f32::from_bits(self.integral_error.load(Ordering::Relaxed));

        // PI control - update integral error
        let new_integral_error = current_integral_error + error;
        self.integral_error.store(new_integral_error.to_bits(), Ordering::Relaxed);

        let mut correction =
            self.kp * (error / target) + self.ki * (new_integral_error / target);

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
