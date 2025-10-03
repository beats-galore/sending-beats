// Layer 2: Input Processing Workers
//
// Each input device gets its own dedicated worker thread that:
// 1. Receives raw audio from Layer 1 input capture
// 2. Resamples to maximum sample rate (e.g., 48kHz)
// 3. Applies per-input effects (EQ, compressor, etc.)
// 4. Sends processed audio to Layer 3 mixing

use anyhow::Result;
use colored::*;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, Notify};
use tracing::{error, info, warn};

use super::audio_worker::{AudioWorker, AudioWorkerState};
use crate::audio::effects::{CustomAudioEffectsChain, DefaultAudioEffectsChain};
use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use crate::audio::mixer::resampling::RubatoSRC;
use crate::audio::mixer::stream_management::virtual_mixer::VirtualMixer;
use crate::audio::VUChannelService;

/// Input processing worker for a specific device
pub struct InputWorker {
    state: AudioWorkerState,

    channel_number: u32,
    samples_processed: u64,
    processing_time_total: std::time::Duration,

    default_effects: Arc<Mutex<DefaultAudioEffectsChain>>,
    custom_effects: CustomAudioEffectsChain,
    any_channel_solo: Arc<std::sync::atomic::AtomicBool>,
}

impl InputWorker {
    pub fn new_with_rtrb(
        device_id: String,
        device_sample_rate: u32,
        target_sample_rate: u32,
        channels: u16,
        chunk_size: usize,
        rtrb_consumer: rtrb::Consumer<f32>,
        rtrb_producer: rtrb::Producer<f32>,
        channel_number: u32,
        any_channel_solo: Arc<std::sync::atomic::AtomicBool>,
        hardware_queue_tracker: AtomicQueueTracker,
        mixing_queue_tracker: AtomicQueueTracker,
        initial_gain: Option<f32>,
        initial_pan: Option<f32>,
        initial_muted: Option<bool>,
        initial_solo: Option<bool>,
    ) -> Self {
        info!(
            "🎤 {}: Creating worker for device '{}' ({} Hz → {} Hz, {} channels, channel #{})",
            "INPUT_WORKER".on_cyan().white(),
            device_id,
            device_sample_rate,
            target_sample_rate,
            channels,
            channel_number
        );

        let mut default_effects = DefaultAudioEffectsChain::new(device_id.clone());

        if let Some(gain) = initial_gain {
            default_effects.set_gain(gain);
            info!(
                "🔊 {}: Initialized gain for '{}' to {}",
                "INPUT_WORKER".on_cyan().white(),
                device_id,
                gain
            );
        }
        if let Some(pan) = initial_pan {
            default_effects.set_pan(pan);
            info!(
                "🎚️ {}: Initialized pan for '{}' to {}",
                "INPUT_WORKER".on_cyan().white(),
                device_id,
                pan
            );
        }
        if let Some(muted) = initial_muted {
            default_effects.set_muted(muted);
            info!(
                "🔇 {}: Initialized muted for '{}' to {}",
                "INPUT_WORKER".on_cyan().white(),
                device_id,
                muted
            );
        }
        if let Some(solo) = initial_solo {
            default_effects.set_solo(solo);
            info!(
                "🎯 {}: Initialized solo for '{}' to {}",
                "INPUT_WORKER".on_cyan().white(),
                device_id,
                solo
            );
        }

        let state = AudioWorkerState::new(
            device_id.clone(),
            device_sample_rate,
            target_sample_rate,
            channels,
            chunk_size,
            rtrb_consumer,
            rtrb_producer,
            mixing_queue_tracker,
        );

        Self {
            state,
            channel_number,
            default_effects: Arc::new(Mutex::new(default_effects)),
            custom_effects: CustomAudioEffectsChain::new(target_sample_rate),
            any_channel_solo,
            samples_processed: 0,
            processing_time_total: std::time::Duration::ZERO,
        }
    }

    pub fn get_stats(&self) -> InputWorkerStats {
        InputWorkerStats {
            device_id: self.state.device_id().to_string(),
            device_sample_rate: self.state.device_sample_rate(),
            target_sample_rate: self.state.target_sample_rate(),
            samples_processed: self.samples_processed,
            is_running: true,
        }
    }

    pub fn channel_number(&self) -> u32 {
        self.channel_number
    }

    pub fn get_default_effects(&self) -> Arc<Mutex<DefaultAudioEffectsChain>> {
        self.default_effects.clone()
    }

    pub fn get_custom_effects_mut(&mut self) -> &mut CustomAudioEffectsChain {
        &mut self.custom_effects
    }
}

impl AudioWorker for InputWorker {
    fn device_id(&self) -> &str {
        self.state.device_id()
    }

    fn device_sample_rate(&self) -> u32 {
        self.state.device_sample_rate()
    }

    fn target_sample_rate(&self) -> u32 {
        self.state.target_sample_rate()
    }

    fn set_target_sample_rate(&mut self, rate: u32) {
        self.state.set_target_sample_rate(rate);
    }

    fn channels(&self) -> u16 {
        self.state.channels()
    }

    fn chunk_size(&self) -> usize {
        self.state.chunk_size()
    }

    fn set_chunk_size(&mut self, size: usize) {
        self.state.set_chunk_size(size);
    }

    fn resampler_mut(&mut self) -> &mut Option<RubatoSRC> {
        self.state.resampler_mut()
    }

    fn set_resampler(&mut self, resampler: Option<RubatoSRC>) {
        self.state.set_resampler(resampler);
    }

    fn queue_tracker(&self) -> &AtomicQueueTracker {
        self.state.queue_tracker()
    }

    fn rtrb_consumer(&self) -> &Arc<Mutex<rtrb::Consumer<f32>>> {
        self.state.rtrb_consumer()
    }

    fn rtrb_producer(&self) -> &Arc<Mutex<rtrb::Producer<f32>>> {
        self.state.rtrb_producer()
    }

    fn set_worker_handle(&mut self, handle: tokio::task::JoinHandle<()>) {
        self.state.set_worker_handle(handle);
    }

    fn take_worker_handle(&mut self) -> Option<tokio::task::JoinHandle<()>> {
        self.state.take_worker_handle()
    }

    fn log_prefix(&self) -> &str {
        "INPUT_WORKER"
    }
}

impl InputWorker {
    pub fn start(
        &mut self,
        vu_channel: Option<tauri::ipc::Channel<crate::audio::VUChannelData>>,
    ) -> Result<()> {
        // Clone state for the post-processing closure
        let default_effects = self.default_effects.clone();
        let any_channel_solo = self.any_channel_solo.clone();
        let channel_number = self.channel_number;
        let channels = self.state.channels();

        let vu_service = vu_channel.map(|channel| {
            info!(
                "{}: VU channel enabled for {}",
                "VU_SETUP".on_cyan().white(),
                self.state.device_id()
            );
            VUChannelService::new(channel, self.state.target_sample_rate(), 8, 60)
        });

        let post_process_fn = move |samples: &mut Vec<f32>,
                                    processing_channels: u16,
                                    _device_id: &str|
              -> Result<()> {
            // Mono-to-stereo conversion
            if channels == 1 && processing_channels == 2 {
                *samples = VirtualMixer::convert_mono_to_stereo(samples);
            }

            // Apply default effects
            let any_solo = any_channel_solo.load(std::sync::atomic::Ordering::Relaxed);
            if let Ok(effects) = default_effects.lock() {
                if processing_channels == 2 {
                    effects.process_stereo_interleaved(samples, any_solo);
                } else {
                    effects.process_mono(samples, any_solo);
                }
            }

            // VU metering
            if let Some(ref vu) = vu_service {
                vu.queue_channel_audio(channel_number, samples);
            }

            Ok(())
        };

        AudioWorker::start_processing_thread(self, Some(post_process_fn))
    }

    pub async fn stop(&mut self) -> Result<()> {
        AudioWorker::stop(self).await
    }

    pub fn update_target_mix_rate(&mut self, target_mix_rate: u32) -> Result<()> {
        self.update_custom_effects(CustomAudioEffectsChain::new(target_mix_rate));
        AudioWorker::update_target_mix_rate(self, target_mix_rate)
    }

    pub fn update_custom_effects(&mut self, new_effects_chain: CustomAudioEffectsChain) {
        self.custom_effects = new_effects_chain;
        info!(
            "🎛️ {}: Updated custom effects chain for device '{}'",
            "INPUT_WORKER".on_cyan().white(),
            self.state.device_id()
        );
    }

    pub fn update_gain(&mut self, gain: f32) {
        if let Ok(mut effects) = self.default_effects.lock() {
            effects.set_gain(gain);
        }
    }

    pub fn update_pan(&mut self, pan: f32) {
        if let Ok(mut effects) = self.default_effects.lock() {
            effects.set_pan(pan);
        }
    }

    pub fn update_muted(&mut self, muted: bool) {
        if let Ok(mut effects) = self.default_effects.lock() {
            effects.set_muted(muted);
        }
    }

    pub fn update_solo(&mut self, solo: bool) {
        if let Ok(mut effects) = self.default_effects.lock() {
            effects.set_solo(solo);
        }
        self.any_channel_solo
            .store(solo, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get processing statistics
    pub fn get_queue_tracker_for_consumer(&self) -> AtomicQueueTracker {
        self.queue_tracker().clone()
    }
}

#[derive(Debug, Clone)]
pub struct InputWorkerStats {
    pub device_id: String,
    pub device_sample_rate: u32,
    pub target_sample_rate: u32,
    pub samples_processed: u64,
    pub is_running: bool,
}
