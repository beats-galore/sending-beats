// Layer 4: Output Processing Workers
//
// Each output device gets its own dedicated worker thread that:
// 1. Receives mixed audio from Layer 3 mixing
// 2. Resamples from max rate to device-specific rate
// 3. Buffers samples to proper chunk sizes for hardware
// 4. Sends audio to actual output devices

use anyhow::Result;
use std::sync::{Arc, Mutex};
use tracing::info;

use super::audio_worker::{AudioWorker, AudioWorkerState};
use crate::audio::mixer::latency_probe::{LatencyProbe, WorkerLatencyGauges};
use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use crate::audio::mixer::resampling::RubatoSRC;
use colored::*;

/// Output processing worker for a specific device
pub struct OutputWorker {
    state: AudioWorkerState,
}

impl OutputWorker {
    pub fn new_with_rtrb_producer_and_tracker(
        device_id: String,
        device_sample_rate: u32,
        target_sample_rate: u32,
        target_chunk_size: usize,
        channels: u16,
        rtrb_consumer: rtrb::Consumer<f32>,
        hardware_rtrb_producer: Option<rtrb::Producer<f32>>,
        hardware_queue_tracker: AtomicQueueTracker,
        _mixing_queue_tracker: AtomicQueueTracker,
        latency_probe: &LatencyProbe,
    ) -> Self {
        let has_hardware_output = hardware_rtrb_producer.is_some();
        info!(
            "🔊 {}: Creating worker for device '{}' ({} Hz → {} Hz, {} sample chunks, hardware: {})",
            "OUTPUT_WORKER".on_blue().yellow(),
            device_id,
            target_sample_rate,
            device_sample_rate,
            target_chunk_size,
            has_hardware_output
        );

        let rtrb_producer_raw = if let Some(hw_prod) = hardware_rtrb_producer {
            hw_prod
        } else {
            let (prod, _) = rtrb::RingBuffer::<f32>::new(1);
            prod
        };

        // OutputWorker receives samples at target_sample_rate (mixing) and outputs at device_sample_rate (hardware)
        // So we swap the rates when initializing AudioWorkerState
        let state = AudioWorkerState::new(
            device_id.clone(),
            target_sample_rate, // Input: mixing rate (e.g., 48kHz)
            device_sample_rate, // Output: hardware rate (e.g., 44.1kHz)
            channels,
            target_chunk_size,
            rtrb_consumer,
            rtrb_producer_raw,
            hardware_queue_tracker,
            WorkerLatencyGauges::for_output(latency_probe, &device_id),
        );

        Self { state }
    }
}

// Implement AudioWorker trait for OutputWorker
impl AudioWorker for OutputWorker {
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
        // The mix is always stereo
        2
    }

    fn outbound_channels(&self) -> u16 {
        self.state.channels()
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
        // One render callback's worth of audio. `target_sample_rate` is the
        // hardware rate on this side, since an output worker's rates are swapped.
        let frames = self.state.chunk_size() / self.state.channels().max(1) as usize;
        std::time::Duration::from_secs_f64(frames as f64 / self.state.target_sample_rate() as f64)
    }

    fn applies_backpressure(&self) -> bool {
        true
    }

    fn log_prefix(&self) -> &str {
        "OUTPUT_WORKER"
    }
}

impl OutputWorker {
    pub fn update_target_mix_rate(&mut self, target_mix_rate: u32) -> Result<()> {
        AudioWorker::update_target_mix_rate(self, target_mix_rate)
    }

    pub fn start(&mut self) -> Result<()> {
        AudioWorker::start_processing_thread(self, None::<fn(&mut Vec<f32>, &str) -> Result<()>>)
    }

    pub async fn stop(&mut self) -> Result<()> {
        AudioWorker::stop(self).await
    }
}
