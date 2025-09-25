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
use tracing::warn;

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

        // Add samples to device buffer
        device_buffer.push_back(samples);

        // Prevent buffer overflow
        if device_buffer.len() > self.max_buffer_samples {
            device_buffer.pop_front();
            warn!(
                "⚠️ {}: Device '{}' buffer overflow, dropping oldest samples",
                "TEMPORAL_BUFFER".red(),
                device_id
            );
        }
    }

    /// Extract synchronized samples from all devices within the sync window
    /// Returns samples that have timestamps within sync_window_ms of each other
    ///
    /// This is the core synchronization logic:
    /// 1. Find oldest sample across all devices
    /// 2. Extract samples from each device within sync_window of that oldest sample
    /// 3. Only return if we have multiple devices OR single device ready
    pub fn extract_synchronized_samples(&mut self) -> Vec<(String, ProcessedAudioSamples)> {
        if self.device_buffers.is_empty() {
            return Vec::new();
        }

        // Find the oldest sample across all devices
        let mut oldest_time: Option<std::time::Instant> = None;
        for (_, buffer) in self.device_buffers.iter() {
            if let Some(oldest_sample) = buffer.front() {
                if oldest_time.is_none() || oldest_sample.timestamp < oldest_time.unwrap() {
                    oldest_time = Some(oldest_sample.timestamp);
                }
            }
        }

        let sync_time = match oldest_time {
            Some(time) => time,
            None => return Vec::new(), // No samples available
        };

        let sync_window = std::time::Duration::from_millis(self.sync_window_ms);
        let mut synchronized_samples = Vec::new();

        // Extract samples from each device that fall within the sync window
        for (device_id, buffer) in self.device_buffers.iter_mut() {
            if let Some(sample) = buffer.front() {
                // Check if this sample is within the sync window
                if sample.timestamp.duration_since(sync_time) <= sync_window {
                    if let Some(extracted_sample) = buffer.pop_front() {
                        synchronized_samples.push((device_id.clone(), extracted_sample));
                    }
                }
            }
        }

        // Only return synchronized samples if we have multiple devices
        // OR if we have a single device with samples ready
        if synchronized_samples.len() > 1
            || (synchronized_samples.len() == 1 && self.device_buffers.len() == 1)
        {
            synchronized_samples
        } else {
            // Put samples back if we don't have enough for synchronization
            for (device_id, sample) in synchronized_samples {
                if let Some(buffer) = self.device_buffers.get_mut(&device_id) {
                    buffer.push_front(sample);
                }
            }
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
