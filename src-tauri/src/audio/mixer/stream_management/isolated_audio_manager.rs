use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::{error, info, warn};

use super::super::types::VirtualMixer;
use crate::audio::effects::{AudioEffectsChain, EQBand};
use crate::audio::types::AudioChannel;
use tokio::sync::{mpsc, oneshot, Mutex, Notify};

// Internal stream_management module imports
use super::audio_input_stream::AudioInputStream;
use super::stream_manager::{AudioMetrics, StreamManager};

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
    AddCoreAudioInputStreamAlternative {
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

/// Isolated Audio Manager - owns audio streams directly, no Arc sharing!
pub struct IsolatedAudioManager {
    input_streams: HashMap<String, AudioInputStream>,
    output_spmc_writers: HashMap<String, Arc<Mutex<Writer<f32>>>>,
    output_device_sample_rates: HashMap<String, u32>, // Track output device sample rates
    // Virtual mixer instance with persistent resamplers
    virtual_mixer: crate::audio::mixer::types::VirtualMixer,
    stream_manager: StreamManager,
    command_rx: mpsc::Receiver<AudioCommand>,
    metrics: AudioMetrics,
    // TRUE EVENT-DRIVEN: Global notification channels for async processing
    global_input_notifier: Arc<Notify>,
    global_output_notifier: Arc<Notify>,
}

impl IsolatedAudioManager {
    /// Check if any input streams have data available for processing
    /// Returns true if at least one stream has samples ready
    fn has_input_data_available(&self) -> bool {
        for input_stream in self.input_streams.values() {
            // RTRB: Check if consumer has samples available (lock-free!)
            if input_stream.has_samples_available() {
                return true;
            }
        }
        false
    }

    /// Check if any output streams need more data (are running low)

    pub async fn new(command_rx: mpsc::Receiver<AudioCommand>) -> Result<Self, anyhow::Error> {
        // Create a VirtualMixer instance with default config
        let mixer_config = crate::audio::types::MixerConfig::default();
        let virtual_mixer = crate::audio::mixer::types::VirtualMixer::new(mixer_config).await?;
        
        Ok(Self {
            input_streams: HashMap::new(),
            output_spmc_writers: HashMap::new(),
            output_device_sample_rates: HashMap::new(),
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
                        // use std::sync::{LazyLock, Mutex as StdMutex};
                        // static OUTPUT_EVENT_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                        // if let Ok(mut count) = OUTPUT_EVENT_COUNT.lock() {
                        //     *count += 1;
                        //     if *count <= 5 || *count % 1000 == 0 {
                        //         info!("⚡ OUTPUT_EVENT [{}]: Processed audio on output demand notification", count);
                        //     }
                        // }
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
            AudioCommand::AddCoreAudioInputStreamAlternative {
                device_id,
                coreaudio_device_id,
                device_name,
                channels,
                producer,
                input_notifier,
                response_tx,
            } => {
                let result = self
                    .handle_add_coreaudio_input_stream_alternative(
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

    /// Continuous audio processing: mix inputs and distribute to outputs
    async fn process_audio(&mut self) {
        // Debug: Log the processing attempt
        use std::sync::{LazyLock, Mutex as StdMutex};
        static DEBUG_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
        if let Ok(mut count) = DEBUG_COUNT.lock() {
            *count += 1;
            if *count <= 10 || *count % 1000 == 0 {
                println!(
                    "🔧 PROCESS_AUDIO [{}]: Called with {} inputs, {} outputs",
                    count,
                    self.input_streams.len(),
                    self.output_spmc_writers.len()
                );
            }
        }

        if self.input_streams.is_empty() {
            // Only skip if no inputs - we'll drain inputs even without outputs
            return;
        }

        // **SAMPLE RATE CONVERSION PIPELINE**: Step 1 - Collect raw samples and sample rates
        let mut input_samples = Vec::<(String, Vec<f32>)>::new();
        let mut input_sample_rates = Vec::<(String, u32)>::new();

        // Collect input sample rates
        for (device_id, input_stream) in &mut self.input_streams {
            input_sample_rates.push((device_id.clone(), input_stream.sample_rate));
        }
        
        // Determine target mix rate using proper function
        let target_mix_rate = self.calculate_target_mix_rate(&input_sample_rates);
        
        // Continue with sample collection
        for (device_id, input_stream) in &mut self.input_streams {
            
            // Get raw samples (before effects - we'll apply effects after sample rate conversion)
            let samples = input_stream.get_samples();
            if !samples.is_empty() {
                input_samples.push((device_id.clone(), samples));
            }
        }

        if input_samples.is_empty() {
            return;
        }

        // **SAMPLE RATE CONVERSION PIPELINE**: Step 2 - Convert all inputs to target mix rate
        let converted_input_samples = self.virtual_mixer.convert_inputs_to_mix_rate(
            input_samples,
            input_sample_rates,
            target_mix_rate,
        );

        // **SAMPLE RATE CONVERSION PIPELINE**: Step 3 - Apply effects to rate-converted samples
        let mut effected_samples = Vec::<(String, Vec<f32>)>::new();
        
        for (device_id, samples) in converted_input_samples {
            if let Some(input_stream) = self.input_streams.get_mut(&device_id) {
                // **EFFECTS FIX**: Create default channel config with effects enabled
                let mut default_channel_config = crate::audio::types::AudioChannel::default();
                default_channel_config.name = format!("Channel for {}", device_id);
                default_channel_config.input_device_id = Some(device_id.clone());
                default_channel_config.effects_enabled = false; // **TEMPORARY**: Disable effects to test raw audio levels

                // Apply effects to the rate-converted samples
                // TODO: We need a way to apply effects to arbitrary sample data, not just from the stream
                // For now, use the samples as-is after rate conversion
                let processed_samples = samples; // TODO: Apply effects here
                
                if !processed_samples.is_empty() {
                    // Debug log for first few audio processing cycles
                    let peak = processed_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    use std::sync::{LazyLock, Mutex as StdMutex};
                    static PROCESS_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
                    if let Ok(mut count) = PROCESS_COUNT.lock() {
                        *count += 1;
                        if *count <= 20 || *count % 1000 == 0 {
                            println!(
                                "🎵 SRC_AUDIO_PROCESSING [{}]: Input '{}' provided {} samples at {} Hz, peak: {:.4}",
                                count,
                                device_id,
                                processed_samples.len(),
                                target_mix_rate,
                                peak
                            );
                        }
                    }

                    effected_samples.push((device_id.clone(), processed_samples));
                }
            }
        }

        // **SAMPLE RATE CONVERSION PIPELINE**: Step 4 - Mix all rate-converted, effected samples
        let mixed_samples = crate::audio::mixer::types::VirtualMixer::mix_input_samples(effected_samples);

        if !mixed_samples.is_empty() {
            // **SAMPLE RATE CONVERSION PIPELINE**: Step 5 - Convert mixed audio to each output device's rate
            // First collect all the output device information to avoid borrowing conflicts
            let output_device_infos: Vec<(String, u32, Arc<Mutex<Writer<f32>>>)> = self
                .output_spmc_writers
                .iter()
                .map(|(device_id, writer)| {
                    let output_device_rate = self.output_device_sample_rates
                        .get(device_id)
                        .copied()
                        .unwrap_or(target_mix_rate);
                    (device_id.clone(), output_device_rate, writer.clone())
                })
                .collect();

            // Now process each output device using the collected information
            for (device_id, output_device_rate, spmc_writer) in output_device_infos {
                if let Ok(mut writer) = spmc_writer.try_lock() {
                    // **PER-OUTPUT CONVERSION**: Convert mixed samples to this device's actual rate
                    let device_samples = self.virtual_mixer.convert_output_to_device_rate(
                        &device_id,
                        mixed_samples.clone(),
                        target_mix_rate,
                        output_device_rate,
                    );

                    // Write rate-converted samples to SPMC queue for hardware stream to read
                    for &sample in &device_samples {
                        writer.write(sample);
                    }

                    // Debug log for output distribution
                    use std::sync::{LazyLock, Mutex as StdMutex};
                    static OUTPUT_COUNT: LazyLock<
                        StdMutex<std::collections::HashMap<String, u64>>,
                    > = LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));
                    if let Ok(mut count_map) = OUTPUT_COUNT.lock() {
                        let count = count_map.entry(device_id.clone()).or_insert(0);
                        *count += 1;
                        if *count <= 20 || *count % 1000 == 0 {
                            let peak = device_samples
                                .iter()
                                .map(|&s| s.abs())
                                .fold(0.0f32, f32::max);
                            println!(
                                "🔊 SRC_AUDIO_OUTPUT [{}]: Sent {} samples to '{}' at {} Hz, peak: {:.4}",
                                count,
                                device_samples.len(),
                                device_id,
                                output_device_rate,
                                peak
                            );
                        }
                    }
                }
            }

            self.metrics.total_samples_processed += mixed_samples.len() as u64;
        }
    }

    #[cfg(target_os = "macos")]
    async fn handle_add_coreaudio_input_stream_alternative(
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

        // **CRITICAL FIX**: Create AudioInputStream wrapper to match CPAL architecture
        // This allows get_samples_for_device() to find CoreAudio streams in input_streams
        // **ADAPTIVE**: Detect device native sample rate for AudioInputStream and buffer calculations
        let native_sample_rate = crate::audio::devices::coreaudio_stream::get_device_native_sample_rate(coreaudio_device_id)?;
        let mut input_stream =
            AudioInputStream::new(device_id.clone(), device_name.clone(), native_sample_rate)?;

        // Create new RTRB pair - consumer goes to AudioInputStream, producer goes to CoreAudio callback
        let buffer_capacity = (native_sample_rate as usize * 2) / 10; // 100ms of stereo samples
        let buffer_capacity = buffer_capacity.max(4096).min(16384);
        let (coreaudio_producer, audio_input_consumer) =
            rtrb::RingBuffer::<f32>::new(buffer_capacity);

        // Replace the consumer in input_stream with our CoreAudio consumer
        input_stream.audio_buffer_consumer = audio_input_consumer;

        // Store the input stream (with consumer) so get_samples_for_device() can find it
        self.input_streams.insert(device_id.clone(), input_stream);

        // Use StreamManager to create and start the CoreAudio input stream as CPAL alternative
        // **ADAPTIVE AUDIO**: No longer pass sample_rate - it will be detected from device
        self.stream_manager.add_coreaudio_input_stream_alternative(
            device_id.clone(),
            coreaudio_device_id,
            device_name,
            channels,
            coreaudio_producer, // Use new producer that connects to AudioInputStream consumer
            self.global_input_notifier.clone(), // CRITICAL FIX: Use global notifier like CPAL
        )?;

        self.metrics.input_streams = self.input_streams.len();
        info!(
            "✅ CoreAudio input stream (CPAL alternative) added and started for device '{}'",
            device_id
        );
        Ok(())
    }

    fn handle_remove_input_stream(&mut self, device_id: String) -> bool {
        let removed = self.input_streams.remove(&device_id).is_some();
        self.stream_manager.remove_stream(&device_id);
        self.metrics.input_streams = self.input_streams.len();
        removed
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
        let native_sample_rate = crate::audio::devices::coreaudio_stream::get_device_native_sample_rate(coreaudio_device.device_id)?;

        // Create SPMC queue for this output device using detected native rate
        let buffer_capacity = (native_sample_rate as usize * 2) / 10; // 100ms of stereo samples
        let buffer_capacity = buffer_capacity.max(4096).min(16384); // Clamp between 4K-16K samples

        let (spmc_reader, spmc_writer) = spmcq::ring_buffer(buffer_capacity);
        let spmc_writer = Arc::new(Mutex::new(spmc_writer));

        // Store the SPMC writer for mixer to send audio data
        self.output_spmc_writers
            .insert(device_id.clone(), spmc_writer);

        // **SAMPLE RATE TRACKING**: Store the output device's ACTUAL DETECTED sample rate
        println!("🔧 OUTPUT_DEVICE_RATE: Storing DETECTED {} Hz for output device '{}'", 
                 native_sample_rate, device_id);
        self.output_device_sample_rates
            .insert(device_id.clone(), native_sample_rate);

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
        if let Some(input_stream) = self.input_streams.get_mut(&device_id) {
            match input_stream.effects_chain.try_lock() {
                Ok(mut effects_guard) => {
                    *effects_guard = effects;
                    Ok(())
                }
                Err(_) => {
                    println!(
                        "⚠️ LOCK_CONTENTION: Failed to acquire effects chain lock for device {}",
                        device_id
                    );
                    // Continue without updating effects - operation succeeds but effects update is skipped
                    Ok(())
                }
            }
        } else {
            Err(anyhow::anyhow!("Input stream not found: {}", device_id))
        }
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

        if let Some(stream) = self.input_streams.get_mut(device_id) {
            let samples = if channel_config.effects_enabled {
                stream.process_with_effects(channel_config)
            } else {
                stream.get_samples()
            };
            // Debug removed to reduce log spam
            samples
        } else {
            // Debug removed to reduce log spam
            Vec::new()
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
        if let Some((_, input_stream)) = self.input_streams.iter().next() {
            info!(
                "🔧 SAMPLE RATE FIX: Found active input stream with sample rate: {} Hz",
                input_stream.sample_rate
            );
            return input_stream.sample_rate;
        }

        // Check active output streams
        // Check output streams - currently not implemented in IsolatedAudioManager
        // TODO: Add output stream tracking if needed

        // Last resort: use default sample rate
        let default_rate = crate::types::DEFAULT_SAMPLE_RATE;
        warn!(
            "🔧 SAMPLE RATE FIX: No active streams found, falling back to default {} Hz",
            default_rate
        );
        default_rate
    }
}
