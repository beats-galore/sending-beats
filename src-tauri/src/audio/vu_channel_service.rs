use colored::*;
use crossbeam_channel::{bounded, Sender, Receiver};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::ipc::Channel;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::audio::effects::{PeakDetector, RmsDetector};
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
            ).await;
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
        let mut channel_peak_detectors = Vec::with_capacity(max_channels);
        let mut channel_rms_detectors = Vec::with_capacity(max_channels);

        for _ in 0..max_channels {
            channel_peak_detectors.push(PeakDetector::new());
            channel_rms_detectors.push(RmsDetector::new(sample_rate));
        }

        let mut master_peak_left = PeakDetector::new();
        let mut master_peak_right = PeakDetector::new();
        let mut master_rms_left = RmsDetector::new(sample_rate);
        let mut master_rms_right = RmsDetector::new(sample_rate);

        let min_send_interval_us = 1_000_000 / emit_rate_hz as u64;
        let last_send = AtomicU64::new(0);

        info!("{}: VU processing thread started", "VU_THREAD".on_blue().cyan());

        while !shutdown.load(Ordering::Relaxed) {
            match sample_rx.recv_timeout(std::time::Duration::from_millis(100)) {
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

                    let peak_left = channel_peak_detectors[channel_idx].process(&left);
                    let rms_left = channel_rms_detectors[channel_idx].process(&left);
                    let (peak_right, rms_right) = if !right.is_empty() {
                        (peak_left, rms_left)
                    } else {
                        (0.0, 0.0)
                    };

                    if Self::should_send(&last_send, min_send_interval_us) {
                        let event = VULevelEvent::new(
                            format!("channel_{}", id),
                            id,
                            Self::to_db(peak_left),
                            Self::to_db(peak_right),
                            Self::to_db(rms_left),
                            Self::to_db(rms_right),
                            !right.is_empty(),
                        );

                        let _ = channel.send(VUChannelData::from_channel(event));
                    }
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

                    let peak_left = master_peak_left.process(&left);
                    let rms_left = master_rms_left.process(&left);
                    let peak_right = master_peak_right.process(&right);
                    let rms_right = master_rms_right.process(&right);

                    if Self::should_send(&last_send, min_send_interval_us) {
                        let event = MasterVULevelEvent::new(
                            Self::to_db(peak_left),
                            Self::to_db(peak_right),
                            Self::to_db(rms_left),
                            Self::to_db(rms_right),
                        );

                        let _ = channel.send(VUChannelData::from_master(event));
                    }
                }
                Err(_) => continue,
            }
        }

        info!("{}: VU processing thread stopped", "VU_THREAD".on_blue().cyan());
    }

    fn to_db(value: f32) -> f32 {
        if value > 0.0 {
            20.0 * value.log10()
        } else {
            -100.0
        }
    }

    fn should_send(last_send: &AtomicU64, min_interval_us: u64) -> bool {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        let last = last_send.load(Ordering::Relaxed);
        if now_us.saturating_sub(last) >= min_interval_us {
            last_send.store(now_us, Ordering::Relaxed);
            true
        } else {
            false
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