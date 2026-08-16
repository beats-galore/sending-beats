// Layer 2: Input Processing Workers
//
// Each input device gets its own dedicated worker thread that:
// 1. Receives raw audio from Layer 1 input capture
// 2. Resamples to maximum sample rate (e.g., 48kHz)
// 3. Applies per-input effects (EQ, compressor, etc.)
// 4. Sends processed audio to Layer 3 mixing

use anyhow::Result;
use colored::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, Notify};
use tracing::{error, info, warn};

use super::audio_worker::{AudioWorker, AudioWorkerState};
use crate::audio::effects::{
    ChannelStripState, DefaultAudioEffectsChain, EQBand, StereoCustomEffects,
};
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
    custom_effects: Arc<Mutex<StereoCustomEffects>>,
    /// Mirrors the chain's on/off switch so the processing thread can skip a
    /// disabled chain without taking the lock. A channel with effects off must
    /// cost nothing — the chain was originally left disconnected because it
    /// dragged the whole pipeline down.
    custom_effects_active: Arc<AtomicBool>,
    any_channel_solo: Arc<AtomicBool>,
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
        any_channel_solo: Arc<AtomicBool>,
        hardware_queue_tracker: AtomicQueueTracker,
        mixing_queue_tracker: AtomicQueueTracker,
        initial_state: Option<ChannelStripState>,
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

        let effects_enabled = initial_state
            .map(|strip| strip.effects_enabled)
            .unwrap_or(false);
        let custom_effects = if let Some(strip) = initial_state {
            default_effects.set_gain(strip.gain);
            default_effects.set_pan(strip.pan);
            default_effects.set_muted(strip.muted);
            default_effects.set_solo(strip.solo);
            default_effects.set_effects_enabled(strip.effects_enabled);
            info!(
                "🎛️ {}: Restored strip for '{}': gain={}, pan={}, muted={}, solo={}, fx={}",
                "INPUT_WORKER".on_cyan().white(),
                device_id,
                strip.gain,
                strip.pan,
                strip.muted,
                strip.solo,
                strip.effects_enabled
            );
            let mut chain = StereoCustomEffects::with_settings(target_sample_rate, strip.chain);
            chain.set_enabled(strip.effects_enabled);
            chain
        } else {
            StereoCustomEffects::new(target_sample_rate)
        };

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
            custom_effects: Arc::new(Mutex::new(custom_effects)),
            custom_effects_active: Arc::new(AtomicBool::new(effects_enabled)),
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
        let custom_effects = self.custom_effects.clone();
        let custom_effects_active = self.custom_effects_active.clone();
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
            let any_solo = any_channel_solo.load(Ordering::Relaxed);
            if let Ok(effects) = default_effects.lock() {
                effects.process_stereo_interleaved(samples, any_solo);
            }

            // Custom chain (EQ, compressor, limiter), after the fader so the
            // limiter protects what actually leaves the channel. The atomic is
            // the cheap gate: a channel with effects off skips even the lock.
            if custom_effects_active.load(Ordering::Relaxed) {
                if let Ok(mut chain) = custom_effects.lock() {
                    chain.process_stereo_interleaved(samples);
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
        // The chain's filter coefficients and envelope timings are derived
        // from the rate; the wrapper rebuilds them without losing the knobs.
        if let Ok(mut chain) = self.custom_effects.lock() {
            chain.set_sample_rate(target_mix_rate);
        }
        AudioWorker::update_target_mix_rate(self, target_mix_rate)
    }

    pub fn update_eq(&mut self, low_db: Option<f32>, mid_db: Option<f32>, high_db: Option<f32>) {
        if let Ok(mut chain) = self.custom_effects.lock() {
            if let Some(gain) = low_db {
                chain.set_eq_gain(EQBand::Low, gain);
            }
            if let Some(gain) = mid_db {
                chain.set_eq_gain(EQBand::Mid, gain);
            }
            if let Some(gain) = high_db {
                chain.set_eq_gain(EQBand::High, gain);
            }
        }
    }

    pub fn update_compressor(
        &mut self,
        threshold_db: Option<f32>,
        ratio: Option<f32>,
        attack_ms: Option<f32>,
        release_ms: Option<f32>,
        enabled: Option<bool>,
    ) {
        if let Ok(mut chain) = self.custom_effects.lock() {
            chain.update_compressor(threshold_db, ratio, attack_ms, release_ms, enabled);
        }
    }

    pub fn update_limiter(&mut self, threshold_db: Option<f32>, enabled: Option<bool>) {
        if let Ok(mut chain) = self.custom_effects.lock() {
            chain.update_limiter(threshold_db, enabled);
        }
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
        // One switch drives the whole strip: pan in the default chain, and the
        // custom chain as a unit. The atomic is written last so the processing
        // thread never sees the gate open before the chain is set.
        if let Ok(mut chain) = self.custom_effects.lock() {
            chain.set_enabled(enabled);
        }
        self.custom_effects_active.store(enabled, Ordering::Relaxed);
    }

    pub fn update_muted(&mut self, muted: bool) {
        if let Ok(mut effects) = self.default_effects.lock() {
            effects.set_muted(muted);
        }
    }

    /// Set this channel's own solo state.
    ///
    /// The shared any-channel-solo flag is deliberately not written here — it
    /// is an OR across every channel, so only the pipeline, which can see all
    /// of them, recomputes it. A worker writing its own state there turned
    /// un-soloing one channel into un-soloing the room.
    pub fn update_solo(&mut self, solo: bool) {
        if let Ok(mut effects) = self.default_effects.lock() {
            effects.set_solo(solo);
        }
    }

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
