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
        }
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
                "TEMPORAL_ADD".blue(),
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
                        "TEMPORAL_BUFFER".red(),
                        device_id,
                        OVERFLOW_LOG_COUNT,
                        buffer_states.join(", ")
                    );
                }
            }
        }
    }

    /// Extract synchronized samples from all devices within the sync window
    /// Returns samples that have timestamps within sync_window_ms of each other
    ///
    /// This is the core synchronization logic:
    /// 1. Find oldest sample across all devices (ignoring stale devices)
    /// 2. Extract samples from each device within sync_window of that oldest sample
    /// 3. Treat devices with only stale samples (>2x sync window old) as inactive
    pub fn extract_synchronized_samples(&mut self) -> Vec<(String, ProcessedAudioSamples)> {
        if self.device_buffers.is_empty() {
            return Vec::new();
        }

        let now = std::time::Instant::now();
        let stale_threshold = std::time::Duration::from_millis(self.sync_window_ms * 2);

        // Find the oldest sample across all devices, but ignore devices with only stale samples
        let mut oldest_time: Option<std::time::Instant> = None;
        for (_, buffer) in self.device_buffers.iter() {
            if let Some(oldest_sample) = buffer.front() {
                // Skip devices that have stale samples (likely silent/inactive)
                if now.duration_since(oldest_sample.timestamp) <= stale_threshold {
                    if oldest_time.is_none() || oldest_sample.timestamp < oldest_time.unwrap() {
                        oldest_time = Some(oldest_sample.timestamp);
                    }
                }
            }
        }

        let sync_time = match oldest_time {
            Some(time) => time,
            None => return Vec::new(), // No fresh samples available
        };

        let sync_window = std::time::Duration::from_millis(self.sync_window_ms);
        let mut synchronized_samples = Vec::new();

        // Extract ALL samples from each device that fall within the sync window
        // This allows proper mixing of devices with different callback rates
        // (e.g., mic at 10ms intervals vs app at 20ms intervals)
        for (device_id, buffer) in self.device_buffers.iter_mut() {
            let mut extracted_count = 0;
            while let Some(sample) = buffer.front() {
                // Calculate time difference (handle both past and future samples)
                let time_diff = if sample.timestamp >= sync_time {
                    sample.timestamp.duration_since(sync_time)
                } else {
                    sync_time.duration_since(sample.timestamp)
                };

                // Check if this sample is within the sync window
                if time_diff <= sync_window {
                    if let Some(extracted_sample) = buffer.pop_front() {
                        extracted_count += 1;
                        synchronized_samples.push((device_id.clone(), extracted_sample));
                    }
                } else {
                    // Sample is outside sync window, stop extracting from this device
                    break;
                }
            }

            // **DIAGNOSTIC**: Log extraction details per device
            if extracted_count > 0 {
                static EXTRACT_LOG_COUNT: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let log_count =
                    EXTRACT_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if log_count < 20 || log_count % 500 == 0 {
                    info!(
                        "📤 {}: Device '{}' extracted {} chunks from sync window",
                        "TEMPORAL_EXTRACT".green(),
                        device_id,
                        extracted_count
                    );
                }
            }
        }

        // Return synchronized samples if we have ANY device ready
        // The mixing layer will handle filling silence for missing devices
        if !synchronized_samples.is_empty() {
            // Log when we have partial sync (some devices not responding)
            if synchronized_samples.len() < self.device_buffers.len() {
                static PARTIAL_SYNC_LOG: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let log_count = PARTIAL_SYNC_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if log_count < 10 || log_count % 500 == 0 {
                    let active_devices: Vec<String> = synchronized_samples
                        .iter()
                        .map(|(id, _)| id.clone())
                        .collect();
                    let inactive_devices: Vec<String> = self
                        .device_buffers
                        .keys()
                        .filter(|id| !active_devices.contains(id))
                        .cloned()
                        .collect();
                    info!(
                        "🔀 {}: Partial sync - active: [{}], inactive: [{}]",
                        "TEMPORAL_PARTIAL_SYNC".yellow(),
                        active_devices.join(", "),
                        inactive_devices.join(", ")
                    );
                }
            }
            synchronized_samples
        } else {
            Vec::new()
        }
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
                "TEMPORAL_BUFFER".red(),
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
