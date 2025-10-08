// Layer 4: Output Processing Workers
//
// Each output device gets its own dedicated worker thread that:
// 1. Receives mixed audio from Layer 3 mixing
// 2. Resamples from max rate to device-specific rate
// 3. Buffers samples to proper chunk sizes for hardware
// 4. Sends audio to actual output devices

use anyhow::Result;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{error, info, trace, warn};

use super::audio_worker::{AudioWorker, AudioWorkerState};
use super::queue_types::MixedAudioSamples;
use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use crate::audio::mixer::resampling::RubatoSRC;
use colored::*;

use rtrb::Producer;

/// Output processing worker for a specific device
pub struct OutputWorker {
    state: AudioWorkerState,

    #[cfg(target_os = "macos")]
    hardware_update_tx: Option<mpsc::Sender<crate::audio::mixer::stream_management::AudioCommand>>,

    chunks_processed: u64,
    samples_output: u64,
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
        mixing_queue_tracker: AtomicQueueTracker,
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
        );

        Self {
            state,
            #[cfg(target_os = "macos")]
            hardware_update_tx: None,
            chunks_processed: 0,
            samples_output: 0,
        }
    }

    #[cfg(target_os = "macos")]
    pub fn new_with_hardware_updates(
        device_id: String,
        device_sample_rate: u32,
        target_sample_rate: u32,
        target_chunk_size: usize,
        channels: u16,
        rtrb_consumer: rtrb::Consumer<f32>,
        hardware_rtrb_producer: Option<rtrb::Producer<f32>>,
        hardware_update_tx: mpsc::Sender<crate::audio::mixer::stream_management::AudioCommand>,
        hardware_queue_tracker: AtomicQueueTracker,
        mixing_queue_tracker: AtomicQueueTracker,
    ) -> Self {
        let has_hardware_output = hardware_rtrb_producer.is_some();
        info!(
            "🔊 {}: Creating worker with hardware updates for device '{}' ({} Hz → {} Hz, {} sample chunks, hardware: {})",
            "OUTPUT_WORKER".on_blue().yellow(),
            device_id, target_sample_rate, device_sample_rate, target_chunk_size, has_hardware_output
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
        );

        Self {
            state,
            hardware_update_tx: Some(hardware_update_tx),
            chunks_processed: 0,
            samples_output: 0,
        }
    }

    pub fn get_stats(&self) -> OutputWorkerStats {
        OutputWorkerStats {
            device_id: self.state.device_id().to_string(),
            device_sample_rate: self.state.device_sample_rate(),
            chunks_processed: self.chunks_processed,
            samples_output: self.samples_output,
            is_running: true,
        }
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

    fn set_worker_handle(&mut self, handle: tokio::task::JoinHandle<()>) {
        self.state.set_worker_handle(handle);
    }

    fn take_worker_handle(&mut self) -> Option<tokio::task::JoinHandle<()>> {
        self.state.take_worker_handle()
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

    /// Get queue tracker for sharing with CoreAudio callback
    pub fn get_queue_tracker_for_consumer(&self) -> AtomicQueueTracker {
        self.queue_tracker().clone()
    }
}

#[derive(Debug, Clone)]
pub struct QueueInfo {
    pub occupancy: usize,
    pub capacity: usize,
    pub usage_percent: f32,
    pub available: usize,
}

#[derive(Debug, Clone)]
pub struct OutputWorkerStats {
    pub device_id: String,
    pub device_sample_rate: u32,
    pub chunks_processed: u64,
    pub samples_output: u64,
    pub is_running: bool,
}
