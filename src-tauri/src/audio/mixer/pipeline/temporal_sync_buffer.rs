// Temporal Synchronization Buffer for Multi-Device Audio Mixing
//
// This module provides temporal alignment of audio samples from multiple input devices.
// Different audio devices have independent hardware callback timings, which can cause
// samples to arrive at different times even when representing the same time period.
//
// The TemporalSyncBuffer solves this by:
// 1. Buffering samples from each device with timestamps
// 2. Extracting samples that fall within a synchronization window
// 3. Ensuring proper temporal alignment before mixing

use colored::*;
use std::collections::{HashMap, VecDeque};
use tracing::{info, warn};

use super::queue_types::ProcessedAudioSamples;
use crate::audio::mixer::queue_manager::DeliveryCadence;

/// Temporal buffer for synchronizing samples from multiple devices
/// Buffers samples until we have temporal alignment across devices
#[derive(Debug)]
pub struct TemporalSyncBuffer {
    /// Buffered samples per device with timestamps
    device_buffers: HashMap<String, VecDeque<ProcessedAudioSamples>>,
    /// Sync window duration (samples within this window are considered "simultaneous")
    sync_window_ms: u64,
    /// Maximum buffer size per device (prevent memory bloat)
    max_buffer_samples: usize,
    /// Track oldest sample timestamp for garbage collection
    oldest_sample_time: Option<std::time::Instant>,
    /// Delivery cadence per device for accumulation decisions
    device_cadences: HashMap<String, DeliveryCadence>,
}

impl TemporalSyncBuffer {
    /// Create new temporal synchronization buffer
    ///
    /// # Arguments
    /// * `sync_window_ms` - Time window in milliseconds for considering samples "simultaneous"
    /// * `max_buffer_samples` - Maximum samples to buffer per device before dropping
    pub fn new(sync_window_ms: u64, max_buffer_samples: usize) -> Self {
        Self {
            device_buffers: HashMap::new(),
            sync_window_ms,
            max_buffer_samples,
            oldest_sample_time: None,
            device_cadences: HashMap::new(),
        }
    }

    /// Update cadence information for a device
    pub fn update_cadence(&mut self, device_id: &str, cadence: DeliveryCadence) {
        // Log every 1000th update per device
        let update_count = cadence.write_count;
        if update_count % 1000 == 0 {
            info!(
                "🎯 {}: Updating cadence for '{}' - chunk: {:.2}ms, interval: {:.2}ms, writes: {}",
                "CADENCE_UPDATE".on_yellow().green(),
                device_id,
                cadence.chunk_duration_ms(),
                cadence.avg_interval_ms,
                cadence.write_count
            );
        }
        self.device_cadences.insert(device_id.to_string(), cadence);
    }

    /// Add samples from a device to the temporal buffer
    pub fn add_samples(&mut self, device_id: String, samples: ProcessedAudioSamples) {
        // Create buffer for new device if needed
        if !self.device_buffers.contains_key(&device_id) {
            self.device_buffers
                .insert(device_id.clone(), VecDeque::new());
        }

        let device_buffer = self.device_buffers.get_mut(&device_id).unwrap();

        // Update oldest sample time tracking
        if self.oldest_sample_time.is_none()
            || (self.oldest_sample_time.is_some()
                && samples.timestamp < self.oldest_sample_time.unwrap())
        {
            self.oldest_sample_time = Some(samples.timestamp);
        }

        // **DIAGNOSTIC**: Log sample addition to identify pitch/speed issues
        static ADD_SAMPLES_LOG_COUNT: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let log_count = ADD_SAMPLES_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if log_count < 20 || log_count % 500 == 0 {
            info!(
                "📥 {}: Device '{}' adding {} samples (channels: {}, buffer depth: {})",
                "TEMPORAL_ADD".on_yellow().green(),
                device_id,
                samples.samples.len(),
                samples.channels,
                device_buffer.len()
            );
        }

        // Add samples to device buffer
        device_buffer.push_back(samples);

        // Prevent buffer overflow
        if device_buffer.len() > self.max_buffer_samples {
            device_buffer.pop_front();

            // Rate-limit logging
            static mut OVERFLOW_LOG_COUNT: u64 = 0;
            unsafe {
                OVERFLOW_LOG_COUNT += 1;
                if OVERFLOW_LOG_COUNT % 100 == 0 {
                    let buffer_states: Vec<String> = self
                        .device_buffers
                        .iter()
                        .map(|(id, buf)| format!("{}: {} chunks", id, buf.len()))
                        .collect();

                    warn!(
                        "⚠️ {}: Device '{}' buffer overflow (#{}).  Buffer states: [{}]",
                        "TEMPORAL_BUFFER".on_yellow().green(),
                        device_id,
                        OVERFLOW_LOG_COUNT,
                        buffer_states.join(", ")
                    );
                }
            }
        }
    }

    /// Calculate the target time duration for mixing based on largest chunk duration
    /// Returns the time duration in milliseconds that should be represented by each device
    fn calculate_target_duration_ms(&self) -> Option<f64> {
        let mut max_chunk_duration = 0.0;
        let mut found_cadence = false;

        for cadence in self.device_cadences.values() {
            if cadence.is_initialized() {
                found_cadence = true;
                let chunk_dur = cadence.chunk_duration_ms();
                if chunk_dur > max_chunk_duration {
                    max_chunk_duration = chunk_dur;
                }
            }
        }

        if found_cadence && max_chunk_duration > 0.0 {
            Some(max_chunk_duration)
        } else {
            None
        }
    }

    /// Check if we have enough accumulated samples from a device for the target duration
    fn has_sufficient_samples(&self, device_id: &str, target_duration_ms: f64) -> bool {
        if let Some(buffer) = self.device_buffers.get(device_id) {
            if let Some(cadence) = self.device_cadences.get(device_id) {
                if !cadence.is_initialized() {
                    return !buffer.is_empty();
                }

                let total_samples: usize = buffer.iter().map(|s| s.samples.len()).sum();
                let samples_per_ms = cadence.samples_per_ms();

                if samples_per_ms > 0.0 {
                    let available_duration_ms = total_samples as f64 / samples_per_ms;
                    available_duration_ms >= target_duration_ms
                } else {
                    !buffer.is_empty()
                }
            } else {
                !buffer.is_empty()
            }
        } else {
            false
        }
    }

    /// Extract synchronized samples from all devices within the sync window
    /// Returns samples that have timestamps within sync_window_ms of each other
    ///
    /// This is the core synchronization logic with cadence awareness:
    /// 1. Find the slowest device's callback interval (target duration)
    /// 2. Accumulate samples from faster devices until they cover the target duration
    /// 3. Extract samples from all devices when each has sufficient accumulated samples
    /// 4. Treat devices with only stale samples (>2x sync window old) as inactive
    pub fn extract_synchronized_samples(&mut self) -> Vec<(String, ProcessedAudioSamples)> {
        if self.device_buffers.is_empty() {
            return Vec::new();
        }

        let now = std::time::Instant::now();

        // Calculate stale threshold based on slowest device's delivery cadence
        // This prevents marking a slow device as stale when it's just between deliveries
        let mut max_delivery_interval_ms = self.sync_window_ms * 2; // Default fallback
        for cadence in self.device_cadences.values() {
            if cadence.is_initialized() {
                // Use the callback interval + buffer for stale detection
                let delivery_interval = cadence.avg_interval_ms * 3.0; // 3x interval as threshold
                if delivery_interval > max_delivery_interval_ms as f64 {
                    max_delivery_interval_ms = delivery_interval as u64;
                }
            }
        }

        let stale_threshold = std::time::Duration::from_millis(max_delivery_interval_ms);

        static STALE_THRESHOLD_LOG: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let log_count = STALE_THRESHOLD_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if log_count < 5 || log_count % 1000 == 0 {
            info!(
                "⏱️ {}: Stale threshold = {}ms (devices: {})",
                "TEMPORAL_STALE".on_yellow().green(),
                max_delivery_interval_ms,
                self.device_cadences.len()
            );
        }

        // Calculate target duration based on slowest device's cadence
        let target_duration = self.calculate_target_duration_ms();

        // Check which devices have samples available
        // For proper synchronization, ALL devices must have buffered samples
        let all_device_ids: Vec<String> = self.device_buffers.keys().cloned().collect();

        for (device_id, buffer) in self.device_buffers.iter() {
            if buffer.is_empty() {
                // If ANY device buffer is empty, don't extract from ANY device yet
                // Wait for all devices to have samples for temporal synchronization
                static EMPTY_BUFFER_LOG: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let log_count = EMPTY_BUFFER_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if log_count % 1000 == 0 {
                    info!(
                        "⏸️ {}: Device '{}' buffer empty, waiting",
                        "TEMPORAL_EMPTY".on_yellow().green(),
                        device_id
                    );
                }
                return Vec::new();
            }

            // Check if device is still delivering (check newest sample, not oldest)
            // Oldest samples naturally age when buffering, we care if NEW samples are arriving
            if let Some(newest_sample) = buffer.back() {
                if now.duration_since(newest_sample.timestamp) > stale_threshold {
                    // Device hasn't delivered new samples recently - mark as inactive
                    static STALE_DEVICE_LOG: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(0);
                    let log_count =
                        STALE_DEVICE_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if log_count % 1000 == 0 {
                        info!(
                            "⏸️ {}: Device '{}' not delivering (last sample: {}ms ago)",
                            "TEMPORAL_STALE_DEVICE".on_yellow().green(),
                            device_id,
                            now.duration_since(newest_sample.timestamp).as_millis()
                        );
                    }
                    return Vec::new();
                }
            }
        }

        // All devices have fresh samples - extract immediately
        let active_device_ids = all_device_ids;

        // All devices have sufficient samples - extract based on target duration
        static EXTRACT_LOG: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let extract_log_count = EXTRACT_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if extract_log_count % 100 == 0 {
            info!(
                "🎯 {}: Extracting from {} devices (extract #{})",
                "TEMPORAL_EXTRACT".on_yellow().green(),
                active_device_ids.len(),
                extract_log_count
            );
        }

        let mut synchronized_samples = Vec::new();

        for device_id in &active_device_ids {
            let buffer = self.device_buffers.get_mut(device_id).unwrap();

            if let Some(target_dur) = target_duration {
                if let Some(cadence) = self.device_cadences.get(device_id) {
                    let target_samples = (target_dur * cadence.samples_per_ms()).ceil() as usize;
                    let mut accumulated_samples = 0;
                    let mut extracted_count = 0;

                    while accumulated_samples < target_samples && !buffer.is_empty() {
                        if let Some(extracted_sample) = buffer.pop_front() {
                            accumulated_samples += extracted_sample.samples.len();
                            extracted_count += 1;
                            synchronized_samples.push((device_id.clone(), extracted_sample));
                        }
                    }

                    static CADENCE_EXTRACT_LOG: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(0);
                    let log_count =
                        CADENCE_EXTRACT_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if log_count < 20 || log_count % 500 == 0 {
                        info!(
                            "📤 {}: Device '{}' extracted {} chunks ({} samples, target {:.0}ms)",
                            "CADENCE_EXTRACT".on_yellow().green(),
                            device_id,
                            extracted_count,
                            accumulated_samples,
                            target_dur
                        );
                    }
                    continue;
                }
            }

            // Fallback: extract one chunk if no cadence info
            if let Some(extracted_sample) = buffer.pop_front() {
                synchronized_samples.push((device_id.clone(), extracted_sample));
            }
        }

        synchronized_samples
    }

    /// Remove old samples that are beyond the sync window to prevent memory bloat
    /// Called periodically to prevent unbounded buffer growth
    pub fn cleanup_old_samples(&mut self) {
        let now = std::time::Instant::now();
        let max_age = std::time::Duration::from_millis(self.sync_window_ms * 3); // Keep 3x sync window

        for (device_id, buffer) in self.device_buffers.iter_mut() {
            while let Some(sample) = buffer.front() {
                if now.duration_since(sample.timestamp) > max_age {
                    buffer.pop_front();
                } else {
                    break; // Samples are ordered by timestamp
                }
            }
        }
    }

    /// Remove a device from the temporal buffer
    /// Called when a device is removed from the pipeline to prevent waiting for samples that will never arrive
    pub fn remove_device(&mut self, device_id: &str) {
        if let Some(buffer) = self.device_buffers.remove(device_id) {
            warn!(
                "🗑️ {}: Removed device '{}' from temporal buffer ({} buffered samples discarded)",
                "TEMPORAL_BUFFER".on_yellow().green(),
                device_id,
                buffer.len()
            );
        }
    }

    /// Get statistics about buffer state
    pub fn get_stats(&self) -> TemporalSyncStats {
        let total_buffered_samples: usize = self.device_buffers.values().map(|b| b.len()).sum();
        let active_devices = self.device_buffers.len();
        let oldest_timestamp = self.oldest_sample_time;

        TemporalSyncStats {
            active_devices,
            total_buffered_samples,
            oldest_timestamp,
            sync_window_ms: self.sync_window_ms,
        }
    }
}

/// Statistics about temporal synchronization buffer state
#[derive(Debug, Clone)]
pub struct TemporalSyncStats {
    pub active_devices: usize,
    pub total_buffered_samples: usize,
    pub oldest_timestamp: Option<std::time::Instant>,
    pub sync_window_ms: u64,
}
