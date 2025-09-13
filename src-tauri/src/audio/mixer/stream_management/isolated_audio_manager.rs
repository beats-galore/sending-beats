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

/// Isolated Audio Manager - manages device lifecycle only
/// **NEW ARCHITECTURE**: InputWorkers read directly from RTRB, no AudioInputStream storage
pub struct IsolatedAudioManager {
    // **REMOVED**: input_streams - InputWorkers read directly from RTRB now
    output_spmc_writers: HashMap<String, Arc<Mutex<Writer<f32>>>>,
    output_device_sample_rates: HashMap<String, u32>, // Track output device sample rates

    // **NEW**: 4-layer audio pipeline replaces VirtualMixer
    audio_pipeline: AudioPipeline,

    // **NEW**: Pipeline input senders (Layer 1 → Layer 2)
    pipeline_input_senders: HashMap<String, mpsc::UnboundedSender<RawAudioSamples>>,

    // Legacy components (will be gradually phased out)
    virtual_mixer: VirtualMixer, // Keep for transition period
    stream_manager: StreamManager,
    command_rx: mpsc::Receiver<AudioCommand>,
    metrics: AudioMetrics,

    // TRUE EVENT-DRIVEN: Global notification channels for async processing
    global_input_notifier: Arc<Notify>,
    global_output_notifier: Arc<Notify>,
}

impl IsolatedAudioManager {
    /// Check if any input streams have data available for processing
    /// Returns true if at least one stream has samples ready OR resampler buffers have enough for output
    fn has_input_data_available(&self) -> bool {
        // **REMOVED**: input_streams no longer exist - InputWorkers handle input directly
        // **NEW**: Check resampler accumulator buffers (legacy functionality)
        self.has_resampler_data_available()
    }

    /// Check if any resampler accumulators have enough samples ready for output delivery
    /// This prevents audio gaps when input queues are empty but resamplers have buffered data
    fn has_resampler_data_available(&self) -> bool {
        // DEBUG: Add comprehensive logging to understand what's happening
        use std::sync::{LazyLock, Mutex as StdMutex};
        static DEBUG_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
        let should_debug = if let Ok(mut count) = DEBUG_COUNT.try_lock() {
            *count += 1;
            *count <= 10 || *count % 100 == 0
        } else {
            false
        };

        // **LOCK-FREE**: Check output resamplers FIRST (most likely to have buffered data in your setup)
        if should_debug {
            println!(
                "🔍 OUTPUT_RESAMPLER_DEBUG: Found {} output resamplers",
                self.virtual_mixer.output_resamplers.len()
            );
        }

        for (device_id, resampler_arc) in self.virtual_mixer.output_resamplers.iter() {
            if let Ok(resampler) = resampler_arc.try_lock() {
                let target_samples = resampler.get_target_chunk_size() * 2; // Stereo samples
                let current_samples = resampler.get_accumulator_size();

                if should_debug {
                    println!("🔍 OUTPUT_RESAMPLER_DEBUG: Device '{}' - accumulator: {}, target: {}, ready: {}",
                             device_id, current_samples, target_samples, current_samples >= target_samples);
                }

                if current_samples >= target_samples && should_debug {
                    println!("🎯 OUTPUT_RESAMPLER_READY: Device '{}' accumulator has {} samples ready (target: {})",
                             device_id, current_samples, target_samples);
                    return true;
                }
            } else if should_debug {
                println!(
                    "🔍 OUTPUT_RESAMPLER_DEBUG: Failed to lock device '{}' output resampler",
                    device_id
                );
            }
        }

        // **LOCK-FREE**: Check input resamplers (less likely in your setup since inputs match mix rate)
        if should_debug {
            println!(
                "🔍 INPUT_RESAMPLER_DEBUG: Found {} input resamplers",
                self.virtual_mixer.input_resamplers.len()
            );
        }

        for (device_id, resampler_arc) in self.virtual_mixer.input_resamplers.iter() {
            if let Ok(resampler) = resampler_arc.try_lock() {
                let target_samples = resampler.get_target_chunk_size() * 2; // Stereo samples
                let current_samples = resampler.get_accumulator_size();

                if should_debug {
                    println!("🔍 INPUT_RESAMPLER_DEBUG: Device '{}' - accumulator: {}, target: {}, ready: {}",
                             device_id, current_samples, target_samples, current_samples >= target_samples);
                }

                if current_samples >= target_samples {
                    println!("🎯 INPUT_RESAMPLER_READY: Device '{}' accumulator has {} samples ready (target: {})",
                             device_id, current_samples, target_samples);
                    return true;
                }
            } else if should_debug {
                println!(
                    "🔍 INPUT_RESAMPLER_DEBUG: Failed to lock device '{}' input resampler",
                    device_id
                );
            }
        }

        if should_debug {
            println!("🔍 RESAMPLER_DEBUG: No resamplers ready - returning false");
        }
        false
    }

    /// Check if any output streams need more data (are running low)

    pub async fn new(command_rx: mpsc::Receiver<AudioCommand>) -> Result<Self, anyhow::Error> {
        // **NEW**: Create 4-layer AudioPipeline with max sample rate of 48kHz
        const MAX_SAMPLE_RATE: u32 = 48000;
        let mut audio_pipeline = AudioPipeline::new(MAX_SAMPLE_RATE);

        // **LEGACY**: Keep VirtualMixer for transition period
        let virtual_mixer = VirtualMixer::new().await?;

        info!(
            "🎧 ISOLATED_AUDIO_MANAGER: Initialized with 4-layer AudioPipeline (max: {} Hz)",
            MAX_SAMPLE_RATE
        );

        Ok(Self {
            output_spmc_writers: HashMap::new(),
            output_device_sample_rates: HashMap::new(),

            // **NEW**: Pipeline architecture
            audio_pipeline,
            pipeline_input_senders: HashMap::new(),

            // **LEGACY**: Keep for transition
            virtual_mixer,
            stream_manager: StreamManager::new(),
            command_rx,
            metrics: AudioMetrics {
                input_streams: 0,
                output_streams: 0,
                total_samples_processed: 0,
                buffer_underruns: 0,
                average_latency_ms: 0.0,
            },
            // TRUE EVENT-DRIVEN: Initialize global notification channels
            global_input_notifier: Arc::new(Notify::new()),
            global_output_notifier: Arc::new(Notify::new()),
        })
    }

    /// Main processing loop for the isolated audio thread
    pub async fn run(&mut self) {
        info!("🎵 Isolated audio manager started - lock-free RTRB architecture");

        // **NEW**: Start the 4-layer audio pipeline
        if let Err(e) = self.audio_pipeline.start().await {
            error!("❌ Failed to start AudioPipeline: {}", e);
            return;
        }
        info!("🚀 PIPELINE: 4-layer AudioPipeline started successfully");

        // **TRUE EVENT-DRIVEN PROCESSING**: Use async notifications instead of polling
        info!("🚀 TRUE EVENT-DRIVEN: Starting async notification-driven audio processing");

        loop {
            tokio::select! {
                // Handle commands (highest priority)
                command = self.command_rx.recv() => {
                    match command {
                        Some(cmd) => {
                            self.handle_command(cmd).await;
                        },
                        None => break, // Channel closed
                    }
                }

                // **TRUE EVENT-DRIVEN**: Process when input data notification arrives
                _ = self.global_input_notifier.notified() => {
                    // DEBUG: Track that we received the notification
                    use std::sync::{LazyLock, Mutex as StdMutex};
                    static INPUT_NOTIFY_RECEIVED: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                    if let Ok(mut count) = INPUT_NOTIFY_RECEIVED.lock() {
                        *count += 1;
                        if *count <= 10 || *count % 100 == 0 {
                            println!("🔔 INPUT_NOTIFICATION_RECEIVED [{}]: Async loop got notified!", count);
                        }
                    }

                    // **ALWAYS CONSUME**: Always drain input buffers to prevent overflow
                    // Process even without outputs (dummy sink behavior)
                    self.process_audio().await;

                    // // Track event-driven processing
                    // static INPUT_EVENT_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                    // if let Ok(mut count) = INPUT_EVENT_COUNT.lock() {
                    //     *count += 1;
                    //     if *count <= 5 || *count % 100 == 0 {
                    //         let output_status = if self.output_streams.is_empty() { "DUMMY_SINK" } else { "REAL_OUTPUT" };
                    //         info!("⚡ INPUT_EVENT [{}]: Processed audio on input data notification ({})", count, output_status);
                    //     }
                    // }
                }

                // **TRUE EVENT-DRIVEN**: Process when output demand notification arrives
                _ = self.global_output_notifier.notified() => {
                    if self.has_input_data_available() {
                        // **RESPONSIVE PROCESSING**: Output needs data and input has it
                        self.process_audio().await;

                        // Track event-driven processing
                        use std::sync::{LazyLock, Mutex as StdMutex};
                        static OUTPUT_EVENT_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                        if let Ok(mut count) = OUTPUT_EVENT_COUNT.lock() {
                            *count += 1;
                            if *count <= 5 || *count % 1000 == 0 {
                                info!("⚡ OUTPUT_EVENT [{}]: Processed audio on output demand notification", count);
                            }
                        }
                    }
                }


            }
        }
    }

    async fn handle_command(&mut self, command: AudioCommand) {
        match command {
            AudioCommand::RemoveInputStream {
                device_id,
                response_tx,
            } => {
                let result = self.handle_remove_input_stream(device_id);
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

    /// **TEMPORARY**: Process audio using legacy VirtualMixer (pipeline under development)
    async fn process_audio(&mut self) {
        // **ARCHITECTURE NOTE**: This method currently creates inefficient double-buffering
        // by reading from RTRB → AudioInputStream → process_audio → pipeline input queues.
        //
        // TODO: InputWorkers should read directly from RTRB consumers, eliminating this middleman.
        // For now, using legacy VirtualMixer processing until pipeline output is connected.

        use std::sync::{LazyLock, Mutex as StdMutex};
        static PROCESS_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
        if let Ok(mut count) = PROCESS_COUNT.lock() {
            *count += 1;
            if *count <= 10 || *count % 1000 == 0 {
                info!(
                    "🔧 LEGACY_PROCESSING [{}]: Using VirtualMixer ({} outputs)",
                    count,
                    self.output_spmc_writers.len()
                );
            }
        }

        // **REMOVED**: No longer check input_streams since InputWorkers handle input directly

        // **LEGACY PATH**: Use existing VirtualMixer processing for now
        let (input_samples, input_sample_rates) = self.collect_input_sample_data();

        if input_samples.is_empty() {
            return;
        }

        // **ADAPTIVE MIXER**: Determine the target mixing rate from the maximum input sample rate
        let target_mix_rate = input_sample_rates
            .iter()
            .map(|(_, rate)| *rate)
            .max()
            .unwrap_or(48000);

        // **SAMPLE RATE CONVERSION PIPELINE**: Step 2 - Convert all inputs to target mix rate
        let converted_input_samples = self.virtual_mixer.convert_inputs_to_mix_rate(
            input_samples,
            input_sample_rates,
            target_mix_rate,
        );

        // **SAMPLE RATE CONVERSION PIPELINE**: Step 3 - Apply effects to rate-converted samples
        let mut effected_samples = Vec::<(String, Vec<f32>)>::new();

        // **REMOVED**: input_streams no longer exist - InputWorkers handle processing
        effected_samples = converted_input_samples;

        // **SAMPLE RATE CONVERSION PIPELINE**: Step 4 - Mix all rate-converted, effected samples
        let mixed_samples = VirtualMixer::mix_input_samples(effected_samples);

        if !mixed_samples.is_empty() {
            // **SAMPLE RATE CONVERSION PIPELINE**: Step 5 - Convert mixed audio to each output device's rate
            let output_device_infos: Vec<(String, u32, Arc<Mutex<Writer<f32>>>)> = self
                .output_spmc_writers
                .iter()
                .map(|(device_id, writer)| {
                    let output_device_rate = self
                        .output_device_sample_rates
                        .get(device_id)
                        .copied()
                        .unwrap_or(target_mix_rate);
                    (device_id.clone(), output_device_rate, writer.clone())
                })
                .collect();

            for (device_id, output_device_rate, spmc_writer) in output_device_infos {
                if let Ok(mut writer) = spmc_writer.try_lock() {
                    let device_samples = self.virtual_mixer.convert_output_to_device_rate(
                        &device_id,
                        mixed_samples.clone(),
                        target_mix_rate,
                        output_device_rate,
                    );

                    for &sample in &device_samples {
                        writer.write(sample);
                    }
                }
            }

            self.metrics.total_samples_processed += mixed_samples.len() as u64;
        }
    }

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
          "🎤 Adding CoreAudio input stream (CPAL alternative) for device '{}' (CoreAudio ID: {})",
          device_id, coreaudio_device_id
      );

        // **REMOVED AUDIOINPUTSTREAM**: InputWorkers read directly from RTRB
        // **ADAPTIVE**: Detect device native sample rate for buffer calculations
        let native_sample_rate =
            crate::audio::devices::coreaudio_stream::get_device_native_sample_rate(
                coreaudio_device_id,
            )?;

        // Create new RTRB pair - consumer goes to AudioInputStream, producer goes to CoreAudio callback
        let buffer_capacity = (native_sample_rate as usize * 2) / 10; // 100ms of stereo samples
        let buffer_capacity = buffer_capacity.max(4096).min(16384);
        let (coreaudio_producer, audio_input_consumer) =
            rtrb::RingBuffer::<f32>::new(buffer_capacity);

        // **NEW ARCHITECTURE**: Give RTRB consumer directly to InputWorker
        // Create dedicated notification for this input device
        let input_device_notifier = Arc::new(Notify::new());

        // Connect RTRB consumer directly to pipeline InputWorker
        if let Err(e) = self.audio_pipeline.add_input_device_with_consumer(
            device_id.clone(),
            native_sample_rate,
            channels,
            audio_input_consumer,
            input_device_notifier.clone(),
        ) {
            error!(
                "❌ PIPELINE: Failed to connect RTRB consumer to InputWorker for '{}': {}",
                device_id, e
            );
            return Err(e);
        }

        // **REMOVED**: No longer store AudioInputStream - InputWorkers handle RTRB directly
        // Device management is handled by the AudioPipeline now

        // Use StreamManager to create and start the CoreAudio input stream as CPAL alternative
        // **ADAPTIVE AUDIO**: No longer pass sample_rate - it will be detected from device
        self.stream_manager.add_coreaudio_input_stream(
            device_id.clone(),
            coreaudio_device_id,
            device_name,
            channels,
            coreaudio_producer, // Producer writes to RTRB that InputWorker reads from
            input_device_notifier, // Each InputWorker gets its own dedicated notification
        )?;

        info!(
            "✅ CoreAudio input stream (CPAL alternative) added and started for device '{}'",
            device_id
        );
        Ok(())
    }

    fn handle_remove_input_stream(&mut self, device_id: String) -> bool {
        // **NEW PIPELINE**: Remove device from AudioPipeline
        // TODO: Add remove_input_device method to AudioPipeline

        // **LEGACY**: Remove from pipeline input senders (will be cleaned up)
        if self.pipeline_input_senders.remove(&device_id).is_some() {
            info!(
                "✅ PIPELINE: Disconnected input device '{}' from pipeline",
                device_id
            );
        }

        self.stream_manager.remove_stream(&device_id);
        true // Always return true since we don't track input_streams anymore
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
            .insert(device_id.clone(), spmc_writer);

        // **SAMPLE RATE TRACKING**: Store the output device's ACTUAL DETECTED sample rate
        println!(
            "🔧 OUTPUT_DEVICE_RATE: Storing DETECTED {} Hz for output device '{}'",
            native_sample_rate, device_id
        );
        self.output_device_sample_rates
            .insert(device_id.clone(), native_sample_rate);

        // **NEW PIPELINE**: Connect this output device to AudioPipeline Layer 4
        let chunk_size = (native_sample_rate as usize) / 100; // 10ms chunks
        if let Err(e) =
            self.audio_pipeline
                .add_output_device(device_id.clone(), native_sample_rate, chunk_size)
        {
            error!(
                "❌ PIPELINE: Failed to connect output device '{}' to Layer 4: {}",
                device_id, e
            );
        } else {
            info!(
                "✅ PIPELINE: Connected output device '{}' to Layer 4 at {} Hz",
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

    /// **HELPER**: Collect input sample data from all input streams
    fn collect_input_sample_data(&mut self) -> (Vec<(String, Vec<f32>)>, Vec<(String, u32)>) {
        let mut input_samples = Vec::new();
        let mut input_sample_rates = Vec::new();

        // Collect sample rates first
        // **REMOVED**: input_streams no longer exist - InputWorkers read directly from RTRB
        // This method should no longer be called with the new architecture

        (input_samples, input_sample_rates)
    }

    /// **LEGACY**: Connect input device to AudioPipeline Layer 1 (old inefficient way)
    fn connect_input_device_to_pipeline(
        &mut self,
        device_id: &str,
        sample_rate: u32,
    ) -> Result<()> {
        // Connect device to AudioPipeline Layer 1 and get sender
        match self
            .audio_pipeline
            .add_input_device(device_id.to_string(), sample_rate)
        {
            Ok(sender) => {
                // Store sender for future use
                self.pipeline_input_senders
                    .insert(device_id.to_string(), sender);
                info!(
                    "✅ PIPELINE: Connected device '{}' to Layer 1 at {} Hz",
                    device_id, sample_rate
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "❌ PIPELINE: Failed to add input device '{}' to Layer 1: {}",
                    device_id, e
                );
                Err(e)
            }
        }
    }

    /// **NEW PIPELINE**: Get processed audio from Layer 4 and distribute to output devices
    /// TODO: Implement proper output sample retrieval from pipeline Layer 4
    fn distribute_pipeline_output_to_devices(&mut self) {
        // **TODO**: This method needs to be implemented once the pipeline Layer 4
        // provides an API to retrieve processed output samples.
        // For now, this is a placeholder to avoid compilation errors.

        // The pipeline workers will handle output distribution internally
        // through their own channels and processing loops.

        // Debug log to track when this would be called
        use std::sync::{LazyLock, Mutex as StdMutex};
        static CALL_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
        if let Ok(mut count) = CALL_COUNT.lock() {
            *count += 1;
            if *count <= 5 || *count % 1000 == 0 {
                debug!(
                    "🔧 PIPELINE_OUTPUT_PLACEHOLDER [{}]: Would distribute to {} outputs",
                    count,
                    self.output_spmc_writers.len()
                );
            }
        }
    }

    /// Calculate the target mix rate as the highest sample rate among all inputs and outputs
    fn calculate_target_mix_rate(&self, input_sample_rates: &[(String, u32)]) -> u32 {
        let mut max_rate = 0u32;

        // Rate-limited debug logging
        use std::sync::{LazyLock, Mutex as StdMutex};
        static DEBUG_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
        let should_log = if let Ok(mut count) = DEBUG_COUNT.try_lock() {
            *count += 1;
            *count <= 3 || *count % 1000 == 0 // Only first 3 times, then every 1000 times
        } else {
            false
        };

        // Consider all input sample rates
        for (device_id, rate) in input_sample_rates {
            if should_log {
                println!("🎯 INPUT_RATE: Device '{}' at {} Hz", device_id, rate);
            }
            max_rate = max_rate.max(*rate);
        }

        // Consider all output sample rates
        for (device_id, rate) in &self.output_device_sample_rates {
            if should_log {
                println!("🎯 OUTPUT_RATE: Device '{}' at {} Hz", device_id, rate);
            }
            max_rate = max_rate.max(*rate);
        }

        let target_rate = if max_rate == 0 {
            crate::types::DEFAULT_SAMPLE_RATE
        } else {
            max_rate
        };

        if should_log {
            println!("🎯 TARGET_MIX_RATE: Calculated {} Hz", target_rate);
        }
        target_rate
    }

    /// Get the actual hardware sample rate from active audio streams
    /// This fixes sample rate mismatch issues by using real hardware rates instead of mixer config
    pub async fn get_actual_hardware_sample_rate(&self) -> u32 {
        // Check active input streams first - they reflect actual hardware capture rates
        // **REMOVED**: input_streams no longer exist
        let default_rate = 48000u32; // Default fallback
        default_rate
    }

    // if let Some((_, input_stream)) = self.input_streams.iter().next() {
    //     info!(
    //         "🔧 SAMPLE RATE FIX: Found active input stream with sample rate: {} Hz",
    //         input_stream.sample_rate
    //     );
    //     return input_stream.sample_rate;
    // }

    // // Check active output streams
    // // Check output streams - currently not implemented in IsolatedAudioManager
    // // TODO: Add output stream tracking if needed

    // // Last resort: use default sample rate
    // let default_rate = crate::types::DEFAULT_SAMPLE_RATE;
    // warn!(
    //     "🔧 SAMPLE RATE FIX: No active streams found, falling back to default {} Hz",
    //     default_rate
    // );
    // default_rate
}
