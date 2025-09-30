use colored::*;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::ipc::Channel;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::audio::events::{MasterVULevelEvent, VUChannelData, VULevelEvent};

enum VUSample {
    Channel { id: u32, samples: Arc<[f32]> },
    Master { samples: Arc<[f32]> },
}

pub struct VUChannelService {
    sample_tx: Sender<VUSample>,
    processing_handle: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
}

impl VUChannelService {
    pub fn new(
        channel: Channel<VUChannelData>,
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
        channel: Channel<VUChannelData>,
        sample_rate: u32,
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

                            let mut left = Vec::with_capacity(samples.len() / 2);
                            let mut right = Vec::with_capacity(samples.len() / 2);

                            for (i, &sample) in samples.iter().enumerate() {
                                if i % 2 == 0 {
                                    left.push(sample);
                                } else {
                                    right.push(sample);
                                }
                            }

                            let peak_left = Self::calculate_peak(&left);
                            let rms_left = Self::calculate_rms(&left);
                            let (peak_right, rms_right) = if !right.is_empty() {
                                (Self::calculate_peak(&right), Self::calculate_rms(&right))
                            } else {
                                (0.0, 0.0)
                            };

                            drained_count += 1;

                            latest_channel_levels[channel_idx] =
                                Some((peak_left, rms_left, peak_right, rms_right));
                        }
                        Ok(VUSample::Master { samples }) => {
                            let mut left = Vec::with_capacity(samples.len() / 2);
                            let mut right = Vec::with_capacity(samples.len() / 2);

                            for (i, &sample) in samples.iter().enumerate() {
                                if i % 2 == 0 {
                                    left.push(sample);
                                } else {
                                    right.push(sample);
                                }
                            }

                            let peak_left = Self::calculate_peak(&left);
                            let rms_left = Self::calculate_rms(&left);
                            let peak_right = Self::calculate_peak(&right);
                            let rms_right = Self::calculate_rms(&right);

                            drained_count += 1;

                            latest_master_levels =
                                Some((peak_left, rms_left, peak_right, rms_right));
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

                if let Some((peak_left, rms_left, peak_right, rms_right)) = latest_master_levels {
                    let event = MasterVULevelEvent::new(
                        Self::to_db(peak_left),
                        Self::to_db(peak_right),
                        Self::to_db(rms_left),
                        Self::to_db(rms_right),
                    );
                    pending_channel_events.push(VUChannelData::from_master(event));
                }

                for event in pending_channel_events.iter() {
                    let _ = channel.send(event.clone());
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
