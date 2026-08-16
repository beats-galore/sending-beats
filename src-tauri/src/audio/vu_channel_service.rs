use colored::*;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use tauri::ipc::Channel;
use tokio::task::JoinHandle;
use tracing::info;

use crate::audio::events::{BusVULevelEvent, MasterVULevelEvent, VUChannelData, VULevelEvent};

/// The channel every VU service currently reports through.
///
/// Shared rather than cloned into each service because the frontend registers a
/// new channel whenever the webview reloads. A service holding its own clone
/// would keep writing to the channel the previous page left behind, and the
/// meters would stay dead until every device was re-added. Swapping the value
/// here re-points all running services at once.
pub type SharedVUChannel = Arc<RwLock<Option<Channel<VUChannelData>>>>;

pub fn new_shared_vu_channel() -> SharedVUChannel {
    Arc::new(RwLock::new(None))
}

enum VUSample {
    Channel { id: u32, samples: Arc<[f32]> },
    Bus { id: String, samples: Arc<[f32]> },
    Master { samples: Arc<[f32]> },
}

/// Peak and RMS of each side of an interleaved stereo block, in linear scale
///
/// Ordered (peak_left, rms_left, peak_right, rms_right) to match how the
/// processing thread stores what it last measured.
fn stereo_levels(samples: &[f32]) -> (f32, f32, f32, f32) {
    let mut left = Vec::with_capacity(samples.len() / 2);
    let mut right = Vec::with_capacity(samples.len() / 2);

    for (i, &sample) in samples.iter().enumerate() {
        if i % 2 == 0 {
            left.push(sample);
        } else {
            right.push(sample);
        }
    }

    let peak_left = calculate_peak(&left);
    let rms_left = calculate_rms(&left);
    let (peak_right, rms_right) = if right.is_empty() {
        (0.0, 0.0)
    } else {
        (calculate_peak(&right), calculate_rms(&right))
    };

    (peak_left, rms_left, peak_right, rms_right)
}

fn calculate_peak(samples: &[f32]) -> f32 {
    samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max)
}

fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_of_squares: f32 = samples.iter().map(|&s| s * s).sum();
    (sum_of_squares / samples.len() as f32).sqrt()
}

pub struct VUChannelService {
    sample_tx: Sender<VUSample>,
    processing_handle: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
}

impl VUChannelService {
    pub fn new(
        channel: SharedVUChannel,
        sample_rate: u32,
        max_channels: usize,
        emit_rate_hz: u32,
    ) -> Self {
        let (sample_tx, sample_rx) = bounded(256);
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        info!(
            "{}: Starting VU processing thread ({}fps, {} max channels)",
            "VU_INIT".on_blue().cyan(),
            emit_rate_hz,
            max_channels
        );

        let processing_handle = tokio::spawn(async move {
            Self::processing_thread(
                sample_rx,
                channel,
                sample_rate,
                max_channels,
                emit_rate_hz,
                shutdown_clone,
            )
            .await;
        });

        Self {
            sample_tx,
            processing_handle: Some(processing_handle),
            shutdown,
        }
    }

    pub fn queue_channel_audio(&self, channel_id: u32, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        let samples_arc = Arc::from(samples);
        let _ = self.sample_tx.try_send(VUSample::Channel {
            id: channel_id,
            samples: samples_arc,
        });
    }

    /// Meter what a bus handed its outputs this block
    pub fn queue_bus_audio(&self, bus_id: &str, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        let samples_arc = Arc::from(samples);
        let _ = self.sample_tx.try_send(VUSample::Bus {
            id: bus_id.to_string(),
            samples: samples_arc,
        });
    }

    pub fn queue_master_audio(&self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        let samples_arc = Arc::from(samples);
        let _ = self.sample_tx.try_send(VUSample::Master {
            samples: samples_arc,
        });
    }

    async fn processing_thread(
        sample_rx: Receiver<VUSample>,
        channel: SharedVUChannel,
        _sample_rate: u32,
        max_channels: usize,
        emit_rate_hz: u32,
        shutdown: Arc<AtomicBool>,
    ) {
        let send_interval_ms = 1000 / emit_rate_hz as u64;
        let mut last_batch_send = std::time::Instant::now();

        info!(
            "{}: VU processing thread started (batching every {}ms)",
            "VU_THREAD".on_blue().cyan(),
            send_interval_ms
        );

        let mut pending_channel_events: Vec<VUChannelData> = Vec::new();
        let mut latest_channel_levels: Vec<Option<(f32, f32, f32, f32)>> = vec![None; max_channels];
        // Keyed rather than indexed: buses are named and created at runtime, so
        // there is no fixed count to size a slot for each of them
        let mut latest_bus_levels: HashMap<String, (f32, f32, f32, f32)> = HashMap::new();
        let mut latest_master_levels: Option<(f32, f32, f32, f32)> = None;

        while !shutdown.load(Ordering::Relaxed) {
            if last_batch_send.elapsed().as_millis() >= send_interval_ms as u128 {
                let mut drained_count = 0;

                loop {
                    match sample_rx.try_recv() {
                        Ok(VUSample::Channel { id, samples }) => {
                            let channel_idx = id as usize;
                            if channel_idx >= max_channels {
                                continue;
                            }

                            drained_count += 1;
                            latest_channel_levels[channel_idx] = Some(stereo_levels(&samples));
                        }
                        Ok(VUSample::Bus { id, samples }) => {
                            drained_count += 1;
                            latest_bus_levels.insert(id, stereo_levels(&samples));
                        }
                        Ok(VUSample::Master { samples }) => {
                            drained_count += 1;
                            latest_master_levels = Some(stereo_levels(&samples));
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }

                pending_channel_events.clear();

                for (idx, levels) in latest_channel_levels.iter().enumerate() {
                    if let Some((peak_left, rms_left, peak_right, rms_right)) = levels {
                        let event = VULevelEvent::new(
                            format!("channel_{}", idx),
                            idx as u32,
                            Self::to_db(*peak_left),
                            Self::to_db(*peak_right),
                            Self::to_db(*rms_left),
                            Self::to_db(*rms_right),
                            true,
                        );
                        pending_channel_events.push(VUChannelData::from_channel(event));
                    }
                }

                for (bus_id, (peak_left, rms_left, peak_right, rms_right)) in
                    latest_bus_levels.iter()
                {
                    let event = BusVULevelEvent::new(
                        bus_id.clone(),
                        Self::to_db(*peak_left),
                        Self::to_db(*peak_right),
                        Self::to_db(*rms_left),
                        Self::to_db(*rms_right),
                    );
                    pending_channel_events.push(VUChannelData::from_bus(event));
                }

                if let Some((peak_left, rms_left, peak_right, rms_right)) = latest_master_levels {
                    let event = MasterVULevelEvent::new(
                        Self::to_db(peak_left),
                        Self::to_db(peak_right),
                        Self::to_db(rms_left),
                        Self::to_db(rms_right),
                    );
                    pending_channel_events.push(VUChannelData::from_master(event));
                }

                // Read the channel per batch rather than holding one from
                // construction, so a channel registered by a reloaded webview
                // takes over without restarting this thread.
                if let Ok(current) = channel.read() {
                    if let Some(sink) = current.as_ref() {
                        for event in pending_channel_events.iter() {
                            let _ = sink.send(event.clone());
                        }
                    }
                }

                last_batch_send = std::time::Instant::now();
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }

        info!(
            "{}: VU processing thread stopped",
            "VU_THREAD".on_blue().cyan()
        );
    }

    fn to_db(value: f32) -> f32 {
        if value > 1e-10 {
            20.0 * value.log10()
        } else {
            -100.0
        }
    }

    pub async fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.processing_handle.take() {
            let _ = handle.await;
        }
    }
}

impl Drop for VUChannelService {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_levels_read_each_side_separately() {
        // Interleaved: left is full scale, right is half
        let samples = [1.0, 0.5, -1.0, -0.5, 1.0, 0.5, -1.0, -0.5];

        let (peak_left, rms_left, peak_right, rms_right) = stereo_levels(&samples);

        assert_eq!(peak_left, 1.0);
        assert_eq!(peak_right, 0.5);
        assert!((rms_left - 1.0).abs() < 1e-6);
        assert!((rms_right - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_silent_block_reads_as_nothing_rather_than_dividing_by_zero() {
        let (peak_left, rms_left, peak_right, rms_right) = stereo_levels(&[0.0; 8]);

        assert_eq!(
            (peak_left, rms_left, peak_right, rms_right),
            (0.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(stereo_levels(&[]), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn a_block_with_no_right_side_reports_zero_for_it() {
        // A single sample leaves the right side empty, which must not become NaN
        let (peak_left, rms_left, peak_right, rms_right) = stereo_levels(&[0.5]);

        assert_eq!(peak_left, 0.5);
        assert!((rms_left - 0.5).abs() < 1e-6);
        assert_eq!(peak_right, 0.0);
        assert_eq!(rms_right, 0.0);
    }

    #[test]
    fn full_scale_is_zero_db_and_silence_is_floored() {
        assert!((VUChannelService::to_db(1.0) - 0.0).abs() < 1e-6);
        assert!((VUChannelService::to_db(0.5) + 6.0206).abs() < 1e-3);
        assert_eq!(
            VUChannelService::to_db(0.0),
            -100.0,
            "silence floors rather than running to negative infinity"
        );
    }
}
