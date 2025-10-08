// Queue state tracking for SPMC queues that don't expose occupancy data
use colored::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{info, trace, warn};

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

/// Cadence tracking for a device's delivery pattern
#[derive(Debug, Clone)]
pub struct DeliveryCadence {
    pub samples_per_write: usize,
    pub last_write_time: Option<std::time::Instant>,
    pub avg_interval_ms: f64,
    pub write_count: u64,
    pub sample_rate: u32,
    pub device_id: String,
}

impl DeliveryCadence {
    fn new(sample_rate: u32, device_id: String) -> Self {
        Self {
            samples_per_write: 0,
            last_write_time: None,
            avg_interval_ms: 0.0,
            write_count: 0,
            sample_rate,
            device_id,
        }
    }

    fn update(&mut self, sample_count: usize, timestamp: std::time::Instant) {
        self.samples_per_write = sample_count;

        if let Some(last_time) = self.last_write_time {
            let interval_ms = timestamp.duration_since(last_time).as_secs_f64() * 1000.0;

            // Exponential moving average (alpha = 0.1 for smooth averaging)
            if self.write_count > 0 {
                self.avg_interval_ms = self.avg_interval_ms * 0.9 + interval_ms * 0.1;
            } else {
                self.avg_interval_ms = interval_ms;
            }
        }

        self.last_write_time = Some(timestamp);
        self.write_count += 1;
    }

    pub fn is_initialized(&self) -> bool {
        self.write_count >= 3 // Need a few samples to establish pattern
    }

    pub fn samples_per_ms(&self) -> f64 {
        if self.sample_rate > 0 {
            // Calculate based on actual sample rate, not callback interval
            // e.g., 48000 Hz = 48 samples per ms
            self.sample_rate as f64 / 1000.0
        } else {
            0.0
        }
    }

    /// Get the actual audio duration in ms that each chunk represents
    pub fn chunk_duration_ms(&self) -> f64 {
        if self.sample_rate > 0 && self.samples_per_write > 0 {
            // stereo interleaved: divide by 2 for frame count, then convert to ms
            let frames = self.samples_per_write / 2;
            let duration = (frames as f64 / self.sample_rate as f64) * 1000.0;

            static DURATION_LOG: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let log_count = DURATION_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if log_count < 5 {
                info!(
                    "🎯 {}: Cadence duration calc (device: {}): {} samples ({} frames) @ {} Hz = {:.2}ms",
                    "CADENCE_DURATION".on_purple().white(),
                    self.device_id,
                    self.samples_per_write,
                    frames,
                    self.sample_rate,
                    duration,

                );
            }

            duration
        } else {
            static ZERO_LOG: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let log_count = ZERO_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if log_count < 5 {
                warn!(
                    "⚠️ {}: Cadence duration (device: {}) is 0! sample_rate={}, samples_per_write={}",
                    "CADENCE_DURATION".on_purple().white(),
                    self.device_id,
                    self.sample_rate,
                    self.samples_per_write,

                );
            }
            0.0
        }
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

    // Delivery cadence tracking
    cadence: Arc<Mutex<DeliveryCadence>>,
    sample_rate: Arc<AtomicU32>, // Store sample rate for cadence calculations
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
        let device_id = queue_id.clone();
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
            cadence: Arc::new(Mutex::new(DeliveryCadence::new(0, device_id))),
            sample_rate: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Set the sample rate for this tracker (must be called before cadence tracking works properly)
    pub fn set_sample_rate(&self, rate: u32) {
        self.sample_rate.store(rate, Ordering::Relaxed);
        if let Ok(mut cadence) = self.cadence.lock() {
            cadence.sample_rate = rate;
            info!(
                "🎯 {}: Set sample rate for '{}' to {} Hz",
                "QUEUE_SAMPLE_RATE".on_purple().white(),
                self.queue_id,
                rate
            );
        }
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

        // Update cadence tracking
        if let Ok(mut cadence) = self.cadence.lock() {
            let was_initialized = cadence.is_initialized();

            // Debug logging to track sample counts
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

            cadence.update(count, std::time::Instant::now());
            let now_initialized = cadence.is_initialized();

            if !was_initialized && now_initialized {
                info!(
                    "✅ {}: Cadence initialized for '{}' after {} writes",
                    "CADENCE_INIT".green(),
                    self.queue_id,
                    cadence.write_count
                );
            }
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

        let target = (self.capacity / 2) as f32;
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

    /// Get cadence information for this device
    pub fn get_cadence(&self) -> Option<DeliveryCadence> {
        if let Ok(cadence) = self.cadence.lock() {
            if cadence.is_initialized() {
                Some(cadence.clone())
            } else {
                None
            }
        } else {
            None
        }
    }
}
