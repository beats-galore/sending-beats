// Queue types and structures for the 4-layer audio pipeline

use std::collections::HashMap;
use tokio::sync::mpsc;

/// Raw audio samples from input devices (Layer 1 → Layer 2)
#[derive(Debug, Clone)]
pub struct RawAudioSamples {
    pub device_id: String,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub timestamp: std::time::Instant,
}

/// Processed audio samples after resampling and effects (Layer 2 → Layer 3)
#[derive(Debug, Clone)]
pub struct ProcessedAudioSamples {
    pub device_id: String,
    pub samples: Vec<f32>, // Always at max sample rate after Layer 2
    pub channels: u16,
    pub timestamp: std::time::Instant,
    pub effects_applied: bool,
}

/// Mixed audio samples ready for output distribution (Layer 3 → Layer 4)
#[derive(Debug, Clone)]
pub struct MixedAudioSamples {
    pub samples: Vec<f32>, // Stereo samples at max sample rate
    pub sample_rate: u32,  // Max sample rate (e.g., 48kHz)
    pub timestamp: std::time::Instant,
    pub input_count: usize, // How many inputs were mixed
}

/// Audio pipeline queues connecting all layers
#[derive(Debug)]
pub struct PipelineQueues {
    // Layer 1 → Layer 2: Raw input queues (per device)
    pub raw_input_senders: HashMap<String, mpsc::UnboundedSender<RawAudioSamples>>,
    pub raw_input_receivers: HashMap<String, mpsc::UnboundedReceiver<RawAudioSamples>>,

    // Layer 2 → Layer 3: Processed input queues (per device)
    pub processed_input_senders: HashMap<String, mpsc::UnboundedSender<ProcessedAudioSamples>>,
    pub processed_input_receivers: HashMap<String, mpsc::UnboundedReceiver<ProcessedAudioSamples>>,

    // Layer 3 → Layer 4: Mixed audio queue (single stream)
    pub mixed_audio_sender: mpsc::UnboundedSender<MixedAudioSamples>,
    pub mixed_audio_receivers: HashMap<String, mpsc::UnboundedReceiver<MixedAudioSamples>>, // Per output device
}

impl PipelineQueues {
    pub fn new() -> Self {
        let (mixed_tx, _mixed_rx) = mpsc::unbounded_channel();

        Self {
            raw_input_senders: HashMap::new(),
            raw_input_receivers: HashMap::new(),
            processed_input_senders: HashMap::new(),
            processed_input_receivers: HashMap::new(),
            mixed_audio_sender: mixed_tx,
            mixed_audio_receivers: HashMap::new(),
        }
    }

    /// Add a new input device to the pipeline
    pub fn add_input_device(&mut self, device_id: String) -> Result<(), String> {
        if self.raw_input_senders.contains_key(&device_id) {
            return Err(format!("Input device '{}' already exists", device_id));
        }

        // Layer 1 → Layer 2: Raw input queue
        let (raw_tx, raw_rx) = mpsc::unbounded_channel();
        self.raw_input_senders.insert(device_id.clone(), raw_tx);
        self.raw_input_receivers.insert(device_id.clone(), raw_rx);

        // Layer 2 → Layer 3: Processed input queue
        let (processed_tx, processed_rx) = mpsc::unbounded_channel();
        self.processed_input_senders
            .insert(device_id.clone(), processed_tx);
        self.processed_input_receivers
            .insert(device_id.clone(), processed_rx);

        println!(
            "✅ PIPELINE_QUEUE: Added input device '{}' to pipeline",
            device_id
        );
        Ok(())
    }

    /// Add a new output device to the pipeline
    pub fn add_output_device(
        &mut self,
        device_id: String,
    ) -> Result<mpsc::UnboundedReceiver<MixedAudioSamples>, String> {
        if self.mixed_audio_receivers.contains_key(&device_id) {
            return Err(format!("Output device '{}' already exists", device_id));
        }

        // Create a new receiver for this output device from the mixed audio stream
        // Note: This creates a broadcast-style setup where each output gets the same mixed audio
        let (_tx, rx) = mpsc::unbounded_channel();

        println!(
            "✅ PIPELINE_QUEUE: Added output device '{}' to pipeline",
            device_id
        );

        // TODO: We need a broadcast mechanism here - for now return the receiver
        // The mixing layer will need to send to all output device channels
        Ok(rx)
    }

    /// Get sender for raw input (Layer 1 → Layer 2)
    pub fn get_raw_input_sender(
        &self,
        device_id: &str,
    ) -> Option<&mpsc::UnboundedSender<RawAudioSamples>> {
        self.raw_input_senders.get(device_id)
    }

    /// Get sender for processed input (Layer 2 → Layer 3)
    pub fn get_processed_input_sender(
        &self,
        device_id: &str,
    ) -> Option<&mpsc::UnboundedSender<ProcessedAudioSamples>> {
        self.processed_input_senders.get(device_id)
    }

    /// Get receiver for processed input (Layer 2 → Layer 3) - used by MixingLayer
    pub fn take_processed_input_receiver(
        &mut self,
        device_id: &str,
    ) -> Option<mpsc::UnboundedReceiver<ProcessedAudioSamples>> {
        self.processed_input_receivers.remove(device_id)
    }

    /// Get the mixed audio sender (Layer 3 → Layer 4)
    pub fn get_mixed_audio_sender(&self) -> &mpsc::UnboundedSender<MixedAudioSamples> {
        &self.mixed_audio_sender
    }

    /// Remove an input device from the pipeline
    pub fn remove_input_device(&mut self, device_id: String) -> Result<(), String> {
        let mut removed = false;

        if self.raw_input_senders.remove(&device_id).is_some() {
            removed = true;
        }
        if self.raw_input_receivers.remove(&device_id).is_some() {
            removed = true;
        }
        if self.processed_input_senders.remove(&device_id).is_some() {
            removed = true;
        }
        if self.processed_input_receivers.remove(&device_id).is_some() {
            removed = true;
        }

        if !removed {
            return Err(format!("Input device '{}' not found", device_id));
        }

        println!(
            "✅ PIPELINE_QUEUE: Removed input device '{}' from pipeline",
            device_id
        );
        Ok(())
    }
}
