use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use super::virtual_mixer::VirtualMixer;
use crate::audio::effects::{AudioEffectsChain, EQBand};
use crate::audio::mixer::pipeline::queue_types::RawAudioSamples;
use crate::audio::mixer::AudioPipeline;
use crate::audio::types::AudioChannel;
use tokio::sync::{mpsc, oneshot, Mutex, Notify};

// Internal stream_management module imports
use super::stream_manager::{AudioMetrics, StreamManager};
use crate::audio::devices::coreaudio_stream::CoreAudioInputStream;

// Lock-free audio buffer imports
use rtrb::{Consumer, Producer, RingBuffer};
use spmcq::{ring_buffer, ReadResult, Reader, Writer};

// Command channel for isolated audio thread communication
// Cannot derive Debug because Device doesn't implement Debug
pub enum AudioCommand {
    RemoveInputStream {
        device_id: String,
        response_tx: oneshot::Sender<Result<bool>>,
    },
    #[cfg(target_os = "macos")]
    AddCoreAudioOutputStream {
        device_id: String,
        coreaudio_device: crate::audio::types::CoreAudioDevice,
        response_tx: oneshot::Sender<Result<()>>,
    },
    #[cfg(target_os = "macos")]
    AddCoreAudioInputStream {
        device_id: String,
        coreaudio_device_id: coreaudio_sys::AudioDeviceID,
        device_name: String,
        channels: u16,
        producer: Producer<f32>,
        input_notifier: Arc<Notify>,
        response_tx: oneshot::Sender<Result<()>>,
    },
    UpdateEffects {
        device_id: String,
        effects: AudioEffectsChain,
        response_tx: oneshot::Sender<Result<()>>,
    },
    GetVULevels {
        response_tx: oneshot::Sender<HashMap<String, f32>>,
    },
    GetAudioMetrics {
        response_tx: oneshot::Sender<AudioMetrics>,
    },
    GetSamples {
        device_id: String,
        channel_config: crate::audio::types::AudioChannel,
        response_tx: oneshot::Sender<Vec<f32>>,
    },
}

/// Audio System Coordinator - lightweight interface between Tauri commands and audio pipeline
/// **NEW ARCHITECTURE**: AudioPipeline handles all audio processing, this just coordinates
pub struct IsolatedAudioManager {
    // **CORE**: 4-layer audio pipeline handles all audio processing
    audio_pipeline: AudioPipeline,

    // **HARDWARE**: StreamManager handles CoreAudio hardware streams
    stream_manager: StreamManager,

    // **SPMC BRIDGE**: Connect AudioPipeline outputs to hardware inputs
    output_spmc_writers: HashMap<String, Arc<Mutex<Writer<f32>>>>,

    // **COMMAND INTERFACE**: Handle Tauri audio commands
    command_rx: mpsc::Receiver<AudioCommand>,
    metrics: AudioMetrics,

    // **COORDINATION**: Event notifications for pipeline coordination
    global_input_notifier: Arc<Notify>,
    global_output_notifier: Arc<Notify>,
}

impl IsolatedAudioManager {
    /// **REMOVED**: Legacy resampler and VirtualMixer functionality
    /// AudioPipeline now handles all audio processing internally

    pub async fn new(command_rx: mpsc::Receiver<AudioCommand>) -> Result<Self, anyhow::Error> {
        // **CORE**: Create 4-layer AudioPipeline with max sample rate of 48kHz
        const MAX_SAMPLE_RATE: u32 = 48000;
        let audio_pipeline = AudioPipeline::new(MAX_SAMPLE_RATE);

        info!(
            "🎧 AUDIO_COORDINATOR: Initialized with 4-layer AudioPipeline (max: {} Hz)",
            MAX_SAMPLE_RATE
        );

        Ok(Self {
            // **CORE**: Main audio processing pipeline
            audio_pipeline,

            // **HARDWARE**: CoreAudio stream management
            stream_manager: StreamManager::new(),

            // **BRIDGE**: SPMC queues for hardware output
            output_spmc_writers: HashMap::new(),

            // **INTERFACE**: Command handling
            command_rx,
            metrics: AudioMetrics {
                input_streams: 0,
                output_streams: 0,
                total_samples_processed: 0,
                buffer_underruns: 0,
                average_latency_ms: 0.0,
            },

            // **COORDINATION**: Pipeline event notifications
            global_input_notifier: Arc::new(Notify::new()),
            global_output_notifier: Arc::new(Notify::new()),
        })
    }

    /// Main coordination loop - handles commands and coordinates AudioPipeline
    pub async fn run(&mut self) {
        info!("🎵 Audio Coordinator started - coordinating AudioPipeline and hardware");

        // **CORE**: Start the 4-layer audio pipeline
        if let Err(e) = self.audio_pipeline.start().await {
            error!("❌ Failed to start AudioPipeline: {}", e);
            return;
        }
        info!("🚀 PIPELINE: 4-layer AudioPipeline started successfully");

        // **COORDINATION LOOP**: Handle Tauri commands and coordinate components
        loop {
            tokio::select! {
                // Handle Tauri audio commands
                command = self.command_rx.recv() => {
                    match command {
                        Some(cmd) => {
                            self.handle_command(cmd).await;
                        },
                        None => {
                            info!("🛑 AUDIO_COORDINATOR: Command channel closed, shutting down");
                            break;
                        }
                    }
                }
            }
        }

        // Clean shutdown
        if let Err(e) = self.audio_pipeline.stop().await {
            warn!("⚠️ AUDIO_COORDINATOR: Error stopping AudioPipeline: {}", e);
        }
        info!("✅ AUDIO_COORDINATOR: Shut down complete");
    }

    async fn handle_command(&mut self, command: AudioCommand) {
        match command {
            AudioCommand::RemoveInputStream {
                device_id,
                response_tx,
            } => {
                let result = self.handle_remove_input_stream(device_id).await;
                let _ = response_tx.send(Ok(result));
            }
            #[cfg(target_os = "macos")]
            AudioCommand::AddCoreAudioOutputStream {
                device_id,
                coreaudio_device,
                response_tx,
            } => {
                let result = self.add_coreaudio_output_stream_direct(device_id, coreaudio_device);
                let _ = response_tx.send(result);
            }
            #[cfg(target_os = "macos")]
            AudioCommand::AddCoreAudioInputStream {
                device_id,
                coreaudio_device_id,
                device_name,
                channels,
                producer,
                input_notifier,
                response_tx,
            } => {
                let result = self
                    .handle_add_coreaudio_input_stream(
                        device_id,
                        coreaudio_device_id,
                        device_name,
                        channels,
                        producer,
                        input_notifier,
                    )
                    .await;
                let _ = response_tx.send(result);
            }
            AudioCommand::UpdateEffects {
                device_id,
                effects,
                response_tx,
            } => {
                let result = self.handle_update_effects(device_id, effects);
                let _ = response_tx.send(result);
            }
            AudioCommand::GetVULevels { response_tx } => {
                let levels = self.get_vu_levels();
                let _ = response_tx.send(levels);
            }
            AudioCommand::GetAudioMetrics { response_tx } => {
                let metrics = self.get_metrics();
                let _ = response_tx.send(metrics);
            }
            AudioCommand::GetSamples {
                device_id,
                channel_config,
                response_tx,
            } => {
                let samples = self.get_samples_for_device(&device_id, &channel_config);
                let _ = response_tx.send(samples);
            }
        }
    }

    /// **REMOVED**: process_audio() - AudioPipeline handles all audio processing internally

    #[cfg(target_os = "macos")]
    async fn handle_add_coreaudio_input_stream(
        &mut self,
        device_id: String,
        coreaudio_device_id: coreaudio_sys::AudioDeviceID,
        device_name: String,
        channels: u16,
        producer: Producer<f32>,
        input_notifier: Arc<Notify>,
    ) -> Result<()> {
        info!(
            "🎤 AUDIO_COORDINATOR: Adding CoreAudio input stream for device '{}' (ID: {})",
            device_id, coreaudio_device_id
        );

        // **HARDWARE**: Get native sample rate from device
        let native_sample_rate =
            crate::audio::devices::coreaudio_stream::get_device_native_sample_rate(
                coreaudio_device_id,
            )?;

        // **RTRB SETUP**: Create buffer for hardware → AudioPipeline communication
        let buffer_capacity = (native_sample_rate as usize * 2) / 10; // 100ms stereo
        let buffer_capacity = buffer_capacity.max(4096).min(16384);
        let (coreaudio_producer, audio_input_consumer) =
            rtrb::RingBuffer::<f32>::new(buffer_capacity);

        // **PIPELINE INTEGRATION**: Connect RTRB consumer directly to AudioPipeline InputWorker
        let input_device_notifier = Arc::new(Notify::new());
        self.audio_pipeline.add_input_device_with_consumer(
            device_id.clone(),
            native_sample_rate,
            channels,
            audio_input_consumer,
            input_device_notifier.clone(),
        )?;

        // **HARDWARE STREAM**: Create CoreAudio stream that feeds the pipeline
        self.stream_manager.add_coreaudio_input_stream(
            device_id.clone(),
            coreaudio_device_id,
            device_name,
            channels,
            coreaudio_producer,    // Hardware writes to this
            input_device_notifier, // Notifies AudioPipeline when data available
        )?;

        info!(
            "✅ AUDIO_COORDINATOR: Input device '{}' connected to AudioPipeline",
            device_id
        );
        Ok(())
    }

    async fn handle_remove_input_stream(&mut self, device_id: String) -> bool {
        info!(
            "🗑️ AUDIO_COORDINATOR: Removing input device '{}'",
            device_id
        );

        // **PIPELINE**: Remove device from AudioPipeline
        if let Err(e) = self.audio_pipeline.remove_input_device(&device_id).await {
            warn!("⚠️ Failed to remove input device from pipeline: {}", e);
        }

        // **HARDWARE**: Remove hardware stream
        self.stream_manager.remove_stream(&device_id);

        info!("✅ AUDIO_COORDINATOR: Removed input device '{}'", device_id);
        true
    }

    #[cfg(target_os = "macos")]
    fn add_coreaudio_output_stream_direct(
        &mut self,
        device_id: String,
        coreaudio_device: crate::audio::types::CoreAudioDevice,
    ) -> Result<()> {
        info!(
            "🔊 Creating CoreAudio output stream for device '{}' (ID: {})",
            device_id, coreaudio_device.device_id
        );

        // **ADAPTIVE AUDIO**: Detect actual device native sample rate like we do for inputs
        let native_sample_rate =
            crate::audio::devices::coreaudio_stream::get_device_native_sample_rate(
                coreaudio_device.device_id,
            )?;

        // Create SPMC queue for this output device using detected native rate
        let buffer_capacity = (native_sample_rate as usize * 2) / 10; // 100ms of stereo samples
        let buffer_capacity = buffer_capacity.max(4096).min(16384); // Clamp between 4K-16K samples

        let (spmc_reader, spmc_writer) = spmcq::ring_buffer(buffer_capacity);
        let spmc_writer = Arc::new(Mutex::new(spmc_writer));

        // Store the SPMC writer for mixer to send audio data
        self.output_spmc_writers
            .insert(device_id.clone(), spmc_writer.clone());

        info!(
            "🔧 OUTPUT_DEVICE_RATE: Using detected {} Hz for output device '{}'",
            native_sample_rate, device_id
        );

        // **NEW PIPELINE**: Connect this output device to AudioPipeline Layer 4 with SPMC writer
        // Use power-of-2 chunk sizes for optimal performance
        let chunk_size = if native_sample_rate >= 44100 {
            1024 // High sample rates use 1024 samples (~21ms at 48kHz, ~23ms at 44.1kHz)
        } else {
            512 // Lower sample rates use 512 samples
        };
        if let Err(e) = self.audio_pipeline.add_output_device_with_spmc_writer(
            device_id.clone(),
            native_sample_rate,
            chunk_size,
            Some(spmc_writer),
        ) {
            error!(
                "❌ PIPELINE: Failed to connect output device '{}' to Layer 4: {}",
                device_id, e
            );
        } else {
            info!(
                "✅ PIPELINE: Connected output device '{}' to Layer 4 with SPMC writer at {} Hz",
                device_id, native_sample_rate
            );
        }

        // Update the coreaudio_device to use the detected native sample rate
        let mut corrected_coreaudio_device = coreaudio_device;
        corrected_coreaudio_device.sample_rate = native_sample_rate;

        // Create the hardware CoreAudio stream with SPMC reader using corrected sample rate
        self.stream_manager.add_coreaudio_output_stream(
            device_id.clone(),
            corrected_coreaudio_device,
            spmc_reader,
            self.global_output_notifier.clone(),
        )?;

        self.metrics.output_streams = self.output_spmc_writers.len();
        info!(
          "✅ CoreAudio output stream created and started for device '{}' with direct SPMC connection",
          device_id
      );
        Ok(())
    }

    fn handle_update_effects(
        &mut self,
        device_id: String,
        effects: AudioEffectsChain,
    ) -> Result<()> {
        // **REMOVED**: input_streams no longer exist - InputWorkers handle effects directly
        Ok(())
    }

    fn get_vu_levels(&mut self) -> HashMap<String, f32> {
        // TEMPORARY FIX: Disable VU meter buffer draining to test if it's stealing samples
        // VU meters were competing with process_audio() for RTRB consumer access
        HashMap::new()

        // ORIGINAL CODE (commented out for testing):
        // let mut levels = HashMap::new();
        // Get samples from each input stream and calculate VU levels
        // for (device_id, input_stream) in &mut self.input_streams {
        //     let samples = input_stream.get_samples();
        //     if !samples.is_empty() {
        //         // Calculate RMS level for VU meter
        //         let rms =
        //             (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        //         let db_level = if rms > 0.0 { 20.0 * rms.log10() } else { -60.0 };
        //         levels.insert(device_id.clone(), db_level);

        //         self.metrics.total_samples_processed += samples.len() as u64;
        //     }
        // }
        // levels
    }

    fn get_metrics(&self) -> AudioMetrics {
        self.metrics.clone()
    }

    /// Get processed samples from a specific device using lock-free RTRB queues
    fn get_samples_for_device(
        &mut self,
        device_id: &str,
        channel_config: &crate::audio::types::AudioChannel,
    ) -> Vec<f32> {
        // Debug removed

        // **REMOVED**: input_streams no longer exist - InputWorkers handle sampling
        Vec::new() // Method obsolete
                   // let samples = if channel_config.effects_enabled {
                   //     stream.process_with_effects(channel_config)
                   // } else {
                   //     stream.get_samples()
                   // };
                   // // Debug removed to reduce log spam
                   // samples
    }
}
