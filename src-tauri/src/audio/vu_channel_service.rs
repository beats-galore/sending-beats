use colored::*;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::ipc::Channel;
use tracing::{info, warn};

use crate::audio::effects::{PeakDetector, RmsDetector};
use crate::audio::events::{MasterVULevelEvent, VUChannelData, VULevelEvent};

/// High-performance VU level service using Tauri channels for real-time streaming
/// Designed to eliminate the 45+ second delays caused by the event system bottleneck
pub struct VUChannelService {
    channel: Channel<VUChannelData>,

    // Per-channel analyzers
    channel_peak_detectors: Vec<PeakDetector>,
    channel_rms_detectors: Vec<RmsDetector>,

    // Master analyzers
    master_peak_detector_left: PeakDetector,
    master_peak_detector_right: PeakDetector,
    master_rms_detector_left: RmsDetector,
    master_rms_detector_right: RmsDetector,

    // Throttling for channel streaming (less aggressive than events)
    last_send_time: AtomicU64, // Microseconds since epoch
    min_send_interval_us: u64, // Minimum interval between sends in microseconds
}

impl VUChannelService {
    /// Create new VU channel service
    /// channel: Tauri channel for high-performance streaming
    /// emit_rate_hz: How often to send data (can be much higher than events)
    pub fn new(
        channel: Channel<VUChannelData>,
        sample_rate: u32,
        max_channels: usize,
        emit_rate_hz: u32,
    ) -> Self {
        let min_send_interval_us = 1_000_000 / emit_rate_hz as u64; // Convert Hz to microseconds

        info!(
            "{}: Creating VU channel service ({}fps, {} max channels)",
            "VU_CHANNEL_INIT".bright_green(),
            emit_rate_hz,
            max_channels
        );

        // Create analyzers for each channel
        let mut channel_peak_detectors = Vec::with_capacity(max_channels);
        let mut channel_rms_detectors = Vec::with_capacity(max_channels);

        for _ in 0..max_channels {
            channel_peak_detectors.push(PeakDetector::new());
            channel_rms_detectors.push(RmsDetector::new(sample_rate));
        }

        Self {
            channel,
            channel_peak_detectors,
            channel_rms_detectors,
            master_peak_detector_left: PeakDetector::new(),
            master_peak_detector_right: PeakDetector::new(),
            master_rms_detector_left: RmsDetector::new(sample_rate),
            master_rms_detector_right: RmsDetector::new(sample_rate),
            last_send_time: AtomicU64::new(0),
            min_send_interval_us,
        }
    }

    /// Process channel audio and stream VU level data via channel
    /// channel_samples: Interleaved stereo samples [L, R, L, R, ...]
    pub fn process_channel_audio(&mut self, channel_id: u32, channel_samples: &[f32]) {
        if channel_samples.is_empty() {
            return;
        }

        let channel_idx = channel_id as usize;
        if channel_idx >= self.channel_peak_detectors.len() {
            return; // Channel index out of bounds
        }

        // Separate left and right channels
        let mut left_samples = Vec::new();
        let mut right_samples = Vec::new();

        for (i, &sample) in channel_samples.iter().enumerate() {
            if i % 2 == 0 {
                left_samples.push(sample);
            } else {
                right_samples.push(sample);
            }
        }

        // Process with analyzers (use left channel for mono if needed)
        let peak_left = self.channel_peak_detectors[channel_idx].process(&left_samples);
        let rms_left = self.channel_rms_detectors[channel_idx].process(&left_samples);

        let (peak_right, rms_right) = if !right_samples.is_empty() {
            // We'd need separate analyzers for right channel - for now, use left values
            (peak_left, rms_left)
        } else {
            (0.0, 0.0)
        };

        // Convert to dB scale
        let peak_left_db = if peak_left > 0.0 {
            20.0 * peak_left.log10()
        } else {
            -100.0
        };
        let peak_right_db = if peak_right > 0.0 {
            20.0 * peak_right.log10()
        } else {
            -100.0
        };
        let rms_left_db = if rms_left > 0.0 {
            20.0 * rms_left.log10()
        } else {
            -100.0
        };
        let rms_right_db = if rms_right > 0.0 {
            20.0 * rms_right.log10()
        } else {
            -100.0
        };

        // Send data via channel (with light throttling for performance)
        if self.should_send_data() {
            let device_id = format!("channel_{}", channel_id);
            let event = VULevelEvent::new(
                device_id,
                channel_id,
                peak_left_db,
                peak_right_db,
                rms_left_db,
                rms_right_db,
                !right_samples.is_empty(),
            );

            self.send_channel_data(VUChannelData::from_channel(event));
        }
    }

    /// Process master output audio and stream master VU level data
    /// master_samples: Interleaved stereo samples [L, R, L, R, ...]
    pub fn process_master_audio(&mut self, master_samples: &[f32]) {
        if master_samples.is_empty() {
            return;
        }

        // Separate left and right channels
        let mut left_samples = Vec::new();
        let mut right_samples = Vec::new();

        for (i, &sample) in master_samples.iter().enumerate() {
            if i % 2 == 0 {
                left_samples.push(sample);
            } else {
                right_samples.push(sample);
            }
        }

        // Process with master analyzers
        let peak_left = self.master_peak_detector_left.process(&left_samples);
        let rms_left = self.master_rms_detector_left.process(&left_samples);
        let peak_right = self.master_peak_detector_right.process(&right_samples);
        let rms_right = self.master_rms_detector_right.process(&right_samples);

        // Convert to dB scale
        let peak_left_db = if peak_left > 0.0 {
            20.0 * peak_left.log10()
        } else {
            -100.0
        };
        let peak_right_db = if peak_right > 0.0 {
            20.0 * peak_right.log10()
        } else {
            -100.0
        };
        let rms_left_db = if rms_left > 0.0 {
            20.0 * rms_left.log10()
        } else {
            -100.0
        };
        let rms_right_db = if rms_right > 0.0 {
            20.0 * rms_right.log10()
        } else {
            -100.0
        };

        // Send data via channel (with light throttling)
        if self.should_send_data() {
            let event =
                MasterVULevelEvent::new(peak_left_db, peak_right_db, rms_left_db, rms_right_db);
            self.send_channel_data(VUChannelData::from_master(event));
        }
    }

    /// Check if enough time has passed to send data (throttling)
    /// Uses lighter throttling than events since channels can handle higher throughput
    fn should_send_data(&self) -> bool {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        let last_send_us = self.last_send_time.load(Ordering::Relaxed);

        if now_us.saturating_sub(last_send_us) >= self.min_send_interval_us {
            self.last_send_time.store(now_us, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Send VU data via high-performance Tauri channel
    fn send_channel_data(&self, data: VUChannelData) {
        if let Err(e) = self.channel.send(data) {
            warn!(
                "{}: Failed to send VU data via channel: {}",
                "VU_CHANNEL_ERROR".red(),
                e
            );
        }
    }
}
