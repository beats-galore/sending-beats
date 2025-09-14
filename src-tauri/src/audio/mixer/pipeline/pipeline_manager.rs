// Pipeline Manager: Orchestrates the complete 4-layer audio pipeline
//
// Manages the entire audio pipeline lifecycle:
// - Creates and connects all 4 layers
// - Manages input/output device registration
// - Coordinates worker thread lifecycle
// - Provides unified API for audio system integration

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

// SPMC queue imports for hardware output connection
use spmcq::Writer;

use super::{
    input_worker::{InputWorker, InputWorkerStats},
    mixing_layer::{MixingLayer, MixingLayerStats},
    output_worker::{OutputWorker, OutputWorkerStats},
    queue_types::{MixedAudioSamples, PipelineQueues, RawAudioSamples},
};

/// Complete 4-layer audio pipeline manager
pub struct AudioPipeline {
    // Configuration
    max_sample_rate: u32, // Target rate for mixing (e.g., 48kHz)

    // Pipeline components
    queues: PipelineQueues,
    input_workers: HashMap<String, InputWorker>,
    mixing_layer: MixingLayer,
    output_workers: HashMap<String, OutputWorker>,

    // State tracking
    is_running: bool,
    devices_registered: usize,
}

impl AudioPipeline {
    /// Create a new audio pipeline
    pub fn new(max_sample_rate: u32) -> Self {
        info!(
            "🏗️ AUDIO_PIPELINE: Creating new 4-layer audio pipeline (max rate: {} Hz)",
            max_sample_rate
        );

        Self {
            max_sample_rate,
            queues: PipelineQueues::new(),
            input_workers: HashMap::new(),
            mixing_layer: MixingLayer::new(max_sample_rate),
            output_workers: HashMap::new(),
            is_running: false,
            devices_registered: 0,
        }
    }

    fn get_all_sample_rates(&self) -> Vec<(String, u32)> {
        let mut sample_rates = Vec::new();

        // Collect sample rates from input workers
        for (device_id, worker) in &self.input_workers {
            sample_rates.push((device_id.clone(), worker.device_sample_rate));
        }

        // Collect sample rates from output workers
        for (device_id, worker) in &self.output_workers {
            sample_rates.push((device_id.clone(), worker.device_sample_rate));
        }

        sample_rates
    }

    /// Calculate the target mix rate as the highest sample rate among all inputs and outputs
    fn calculate_target_mix_rate(&mut self) -> Result<u32> {
        let all_sample_rates = self.get_all_sample_rates();

        if all_sample_rates.is_empty() {
            return Ok(crate::types::DEFAULT_SAMPLE_RATE);
        }

        if all_sample_rates.len() == 1 {
            return Ok(all_sample_rates[0].1);
        }

        // Find the maximum sample rate using the max function
        let max_rate = all_sample_rates
            .iter()
            .map(|(_, rate)| *rate)
            .max()
            .unwrap_or(crate::types::DEFAULT_SAMPLE_RATE);

        let target_rate = if max_rate == 0 {
            crate::types::DEFAULT_SAMPLE_RATE
        } else {
            max_rate
        };

        self.max_sample_rate = target_rate;
        Ok(target_rate)
    }

    fn update_target_sample_rates(&mut self) -> Result<()> {
        for (device_id, worker) in &mut self.input_workers {
            worker.update_target_mix_rate(self.max_sample_rate);
        }

        // Collect sample rates from output workers
        for (device_id, worker) in &mut self.output_workers {
            worker.update_target_mix_rate(self.max_sample_rate);
        }
        Ok(())
    }

    /// Register a new input device with direct RTRB consumer (bypasses IsolatedAudioManager)
    pub fn add_input_device_with_consumer(
        &mut self,
        device_id: String,
        device_sample_rate: u32,
        channels: u16,
        rtrb_consumer: rtrb::Consumer<f32>,
        input_notifier: Arc<tokio::sync::Notify>,
    ) -> Result<()> {
        if self.input_workers.contains_key(&device_id) {
            return Err(anyhow::anyhow!(
                "Input device '{}' already registered",
                device_id
            ));
        }

        // Add device to queue system first
        self.queues
            .add_input_device(device_id.clone())
            .map_err(|e| anyhow::anyhow!("Failed to add input device: {}", e))?;

        // Get processed input sender for mixing layer
        let processed_output_tx = self
            .queues
            .get_processed_input_sender(&device_id)
            .ok_or_else(|| {
                anyhow::anyhow!("Failed to get processed input sender for {}", device_id)
            })?
            .clone();

        // Connect processed input receiver to mixing layer
        if let Some(processed_input_rx) = self.queues.take_processed_input_receiver(&device_id) {
            self.mixing_layer
                .add_input_stream(device_id.clone(), processed_input_rx);
            info!(
                "✅ PIPELINE: Connected input device '{}' to MixingLayer",
                device_id
            );
        } else {
            warn!(
                "⚠️ PIPELINE: Failed to connect input device '{}' to MixingLayer",
                device_id
            );
        }

        // Create input worker with direct RTRB consumer
        let mut input_worker = InputWorker::new_with_rtrb(
            device_id.clone(),
            device_sample_rate,
            self.max_sample_rate,
            channels,
            rtrb_consumer,
            input_notifier,
            processed_output_tx,
        );

        // every time a new input is added, we have to recalculate the new maximum and update other input workers / output workers.
        self.calculate_target_mix_rate()?;
        self.update_target_sample_rates()?;

        // Update mixing layer with new target rate
        self.mixing_layer
            .update_target_sample_rate(self.max_sample_rate);

        // Start the worker if pipeline is running
        if self.is_running {
            input_worker.start()?;
        }

        self.input_workers.insert(device_id.clone(), input_worker);
        self.devices_registered += 1;

        info!(
            "✅ AUDIO_PIPELINE: Added input device '{}' with direct RTRB consumer ({} Hz → {} Hz)",
            device_id, device_sample_rate, self.max_sample_rate
        );

        Ok(())
    }

    /// Register a new output device with the pipeline
    pub fn add_output_device(
        &mut self,
        device_id: String,
        device_sample_rate: u32,
        chunk_size: usize,
    ) -> Result<()> {
        self.add_output_device_with_spmc_writer(device_id, device_sample_rate, chunk_size, None)
    }

    /// Register a new output device with SPMC writer for hardware connection
    pub fn add_output_device_with_spmc_writer(
        &mut self,
        device_id: String,
        device_sample_rate: u32,
        chunk_size: usize,
        spmc_writer: Option<Arc<tokio::sync::Mutex<spmcq::Writer<f32>>>>,
    ) -> Result<()> {
        if self.output_workers.contains_key(&device_id) {
            return Err(anyhow::anyhow!(
                "Output device '{}' already registered",
                device_id
            ));
        }

        // Create mixed audio receiver for this output device
        let (mixed_tx, mixed_rx) = mpsc::unbounded_channel::<MixedAudioSamples>();

        // Add sender to mixing layer for broadcast
        self.mixing_layer.add_output_sender(mixed_tx);

        // Create output worker for this device (with SPMC writer if provided)
        let mut output_worker = if let Some(spmc_writer) = spmc_writer {
            OutputWorker::new_with_spmc_writer(
                device_id.clone(),
                device_sample_rate,
                chunk_size,
                mixed_rx,
                Some(spmc_writer),
            )
        } else {
            OutputWorker::new(device_id.clone(), device_sample_rate, chunk_size, mixed_rx)
        };

        // Start the worker if pipeline is running
        if self.is_running {
            output_worker.start()?;
        }

        self.output_workers.insert(device_id.clone(), output_worker);
        self.devices_registered += 1;

        // every time a new output is added, we have to recalculate the new maximum and update other input workers / output workers.
        self.calculate_target_mix_rate()?;
        self.update_target_sample_rates()?;

        // Update mixing layer with new target rate
        self.mixing_layer
            .update_target_sample_rate(self.max_sample_rate);

        info!(
            "✅ AUDIO_PIPELINE: Added output device '{}' ({} Hz ← {} Hz, {} sample chunks)",
            device_id, device_sample_rate, self.max_sample_rate, chunk_size
        );

        Ok(())
    }

    /// Start the complete audio pipeline
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }

        info!("🚀 AUDIO_PIPELINE: Starting complete 4-layer pipeline...");

        // Start Layer 2: Input workers
        for (device_id, input_worker) in self.input_workers.iter_mut() {
            if let Err(e) = input_worker.start() {
                error!(
                    "❌ AUDIO_PIPELINE: Failed to start input worker for '{}': {}",
                    device_id, e
                );
                return Err(e);
            }
        }

        // Start Layer 3: Mixing layer
        if let Err(e) = self.mixing_layer.start() {
            error!("❌ AUDIO_PIPELINE: Failed to start mixing layer: {}", e);
            return Err(e);
        }

        // Start Layer 4: Output workers
        for (device_id, output_worker) in self.output_workers.iter_mut() {
            if let Err(e) = output_worker.start() {
                error!(
                    "❌ AUDIO_PIPELINE: Failed to start output worker for '{}': {}",
                    device_id, e
                );
                return Err(e);
            }
        }

        self.is_running = true;

        info!("✅ AUDIO_PIPELINE: Started complete pipeline ({} input workers, 1 mixing layer, {} output workers)",
              self.input_workers.len(), self.output_workers.len());

        Ok(())
    }

    /// Stop the complete audio pipeline
    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            return Ok(());
        }

        info!("🛑 AUDIO_PIPELINE: Stopping complete 4-layer pipeline...");

        // Stop all input workers
        for (device_id, input_worker) in self.input_workers.iter_mut() {
            if let Err(e) = input_worker.stop().await {
                warn!(
                    "⚠️ AUDIO_PIPELINE: Error stopping input worker '{}': {}",
                    device_id, e
                );
            }
        }

        // Stop mixing layer
        if let Err(e) = self.mixing_layer.stop().await {
            warn!("⚠️ AUDIO_PIPELINE: Error stopping mixing layer: {}", e);
        }

        // Stop all output workers
        for (device_id, output_worker) in self.output_workers.iter_mut() {
            if let Err(e) = output_worker.stop().await {
                warn!(
                    "⚠️ AUDIO_PIPELINE: Error stopping output worker '{}': {}",
                    device_id, e
                );
            }
        }

        self.is_running = false;

        info!("✅ AUDIO_PIPELINE: Stopped complete pipeline");

        Ok(())
    }

    /// Send raw audio from Layer 1 to Layer 2
    pub fn send_input_audio(
        &self,
        device_id: &str,
        samples: Vec<f32>,
        sample_rate: u32,
        channels: u16,
    ) -> Result<()> {
        let sender = self
            .queues
            .get_raw_input_sender(device_id)
            .ok_or_else(|| anyhow::anyhow!("No input sender for device '{}'", device_id))?;

        let raw_audio = RawAudioSamples {
            device_id: device_id.to_string(),
            samples,
            sample_rate,
            channels,
            timestamp: std::time::Instant::now(),
        };

        sender.send(raw_audio).map_err(|_| {
            anyhow::anyhow!("Failed to send audio to input worker for '{}'", device_id)
        })?;

        Ok(())
    }

    /// Remove an input device from the pipeline
    pub async fn remove_input_device(&mut self, device_id: &str) -> Result<()> {
        if !self.input_workers.contains_key(device_id) {
            return Err(anyhow::anyhow!("Input device '{}' not found", device_id));
        }

        // Stop the input worker
        if let Some(mut input_worker) = self.input_workers.remove(device_id) {
            // Stop worker gracefully
            if let Err(e) = input_worker.stop().await {
                warn!(
                    "⚠️ AUDIO_PIPELINE: Error stopping input worker '{}': {}",
                    device_id, e
                );
            }
        }

        // Remove from queue system
        self.queues
            .remove_input_device(device_id.to_string())
            .map_err(|e| anyhow::anyhow!("Failed to remove input device from queues: {}", e))?;

        self.devices_registered = self.devices_registered.saturating_sub(1);

        // Recalculate target sample rate and update all workers
        self.calculate_target_mix_rate()?;
        self.update_target_sample_rates()?;

        // Update mixing layer with new target rate
        self.mixing_layer
            .update_target_sample_rate(self.max_sample_rate);

        info!(
            "✅ AUDIO_PIPELINE: Removed input device '{}' and recalculated mix rate to {} Hz",
            device_id, self.max_sample_rate
        );

        Ok(())
    }

    /// Remove an output device from the pipeline
    pub async fn remove_output_device(&mut self, device_id: &str) -> Result<()> {
        if !self.output_workers.contains_key(device_id) {
            return Err(anyhow::anyhow!("Output device '{}' not found", device_id));
        }

        // Stop the output worker
        if let Some(mut output_worker) = self.output_workers.remove(device_id) {
            // Stop worker gracefully
            if let Err(e) = output_worker.stop().await {
                warn!(
                    "⚠️ AUDIO_PIPELINE: Error stopping output worker '{}': {}",
                    device_id, e
                );
            }
        }

        self.devices_registered = self.devices_registered.saturating_sub(1);

        // Recalculate target sample rate and update all workers
        self.calculate_target_mix_rate()?;
        self.update_target_sample_rates()?;

        // Update mixing layer with new target rate
        self.mixing_layer
            .update_target_sample_rate(self.max_sample_rate);

        info!(
            "✅ AUDIO_PIPELINE: Removed output device '{}' and recalculated mix rate to {} Hz",
            device_id, self.max_sample_rate
        );

        Ok(())
    }

    /// Get comprehensive pipeline statistics
    pub fn get_pipeline_stats(&self) -> PipelineStats {
        let input_stats: HashMap<String, InputWorkerStats> = self
            .input_workers
            .iter()
            .map(|(id, worker)| (id.clone(), worker.get_stats()))
            .collect();

        let output_stats: HashMap<String, OutputWorkerStats> = self
            .output_workers
            .iter()
            .map(|(id, worker)| (id.clone(), worker.get_stats()))
            .collect();

        PipelineStats {
            is_running: self.is_running,
            max_sample_rate: self.max_sample_rate,
            input_workers: input_stats,
            mixing_layer: self.mixing_layer.get_stats(),
            output_workers: output_stats,
            total_devices: self.devices_registered,
        }
    }
}

#[derive(Debug)]
pub struct PipelineStats {
    pub is_running: bool,
    pub max_sample_rate: u32,
    pub input_workers: HashMap<String, InputWorkerStats>,
    pub mixing_layer: MixingLayerStats,
    pub output_workers: HashMap<String, OutputWorkerStats>,
    pub total_devices: usize,
}
