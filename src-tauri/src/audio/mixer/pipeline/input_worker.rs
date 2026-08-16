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
use crate::audio::mixer::latency_probe::{LatencyProbe, WorkerLatencyGauges};
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
        latency_probe: &LatencyProbe,
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
            WorkerLatencyGauges::for_input(latency_probe, &device_id),
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

    fn latency_gauges(&self) -> &WorkerLatencyGauges {
        self.state.latency_gauges()
    }

    fn inbound_channels(&self) -> u16 {
        self.state.channels()
    }

    fn outbound_channels(&self) -> u16 {
        // Mono is widened to stereo before it reaches the mixer; anything else
        // arrives with its channel count intact.
        if self.state.channels() == 1 {
            2
        } else {
            self.state.channels()
        }
    }

    fn set_worker_handle(&mut self, handle: std::thread::JoinHandle<()>) {
        self.state.set_worker_handle(handle);
    }

    fn take_worker_handle(&mut self) -> Option<std::thread::JoinHandle<()>> {
        self.state.take_worker_handle()
    }

    fn running(&self) -> &Arc<std::sync::atomic::AtomicBool> {
        self.state.running()
    }

    fn work_period(&self) -> std::time::Duration {
        // One capture callback's worth of audio, at the device's own rate
        let frames = self.state.chunk_size() / self.state.channels().max(1) as usize;
        std::time::Duration::from_secs_f64(frames as f64 / self.state.device_sample_rate() as f64)
    }

    fn log_prefix(&self) -> &str {
        "INPUT_WORKER"
    }
}

impl InputWorker {
    pub fn start(&mut self, vu_channel: crate::audio::SharedVUChannel) -> Result<()> {
        // Clone state for the post-processing closure
        let default_effects = self.default_effects.clone();
        let any_channel_solo = self.any_channel_solo.clone();
        let channel_number = self.channel_number;
        let channels = self.state.channels();

        // Started unconditionally: the frontend may register its channel after
        // this worker exists, and the service simply drops batches until one
        // arrives rather than needing to be created later.
        info!(
            "{}: VU metering enabled for {}",
            "VU_SETUP".on_cyan().white(),
            self.state.device_id()
        );
        let vu_service = Some(VUChannelService::new(
            vu_channel,
            self.state.target_sample_rate(),
            8,
            60,
        ));

        let post_process_fn = move |samples: &mut Vec<f32>, device_id: &str| -> Result<()> {
            // Mono-to-stereo conversion (always convert for mixing layer compatibility)
            if channels == 1 {
                let original_count = samples.len();
                *samples = VirtualMixer::convert_mono_to_stereo(samples);
                let converted_count = samples.len();

                // **DIAGNOSTIC**: Log mono-to-stereo conversion to verify correct sample doubling
                static MONO_STEREO_LOG_COUNT: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let log_count =
                    MONO_STEREO_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if log_count < 20 || log_count % 500 == 0 {
                    info!(
                        "🔄 {}: Device '{}' mono→stereo: {} → {} samples ({}x)",
                        "MONO_TO_STEREO".yellow(),
                        device_id,
                        original_count,
                        converted_count,
                        converted_count as f32 / original_count as f32
                    );
                }
            }

            // Apply default effects (always stereo after conversion above)
            let any_solo = any_channel_solo.load(std::sync::atomic::Ordering::Relaxed);
            if let Ok(effects) = default_effects.lock() {
                effects.process_stereo_interleaved(samples, any_solo);
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

    pub fn update_effects_enabled(&mut self, enabled: bool) {
        if let Ok(mut effects) = self.default_effects.lock() {
            effects.set_effects_enabled(enabled);
        }
    }

    pub fn update_muted(&mut self, muted: bool) {
        if let Ok(mut effects) = self.default_effects.lock() {
            effects.set_muted(muted);
        }
    }

    /// Set this channel's own solo
    ///
    /// Deliberately does not touch `any_channel_solo`. That flag is about every
    /// channel, not this one, and writing this channel's answer into it drops
    /// solo for the whole mix the moment a second soloed channel is turned off.
    /// The pipeline owns it, because the pipeline is what can see them all.
    pub fn update_solo(&mut self, solo: bool) {
        if let Ok(mut effects) = self.default_effects.lock() {
            effects.set_solo(solo);
        }
    }

    /// Whether this channel is soloed, for the aggregate the pipeline keeps
    pub fn is_solo(&self) -> bool {
        self.default_effects
            .lock()
            .map(|effects| effects.is_solo())
            .unwrap_or(false)
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
