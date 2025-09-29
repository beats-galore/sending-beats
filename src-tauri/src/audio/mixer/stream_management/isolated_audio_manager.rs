use anyhow::{Context, Result};
use colored::*;
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
use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use rtrb::{Consumer, Producer, RingBuffer};

// Command channel for isolated audio thread communication
// Cannot derive Debug because Device doesn't implement Debug
pub enum AudioCommand {
    SetAppHandle {
        app_handle: tauri::AppHandle,
        response_tx: oneshot::Sender<Result<()>>,
    },
    SetVUChannel {
        channel: tauri::ipc::Channel<crate::audio::VUChannelData>,
        response_tx: oneshot::Sender<Result<()>>,
    },
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
    UpdateOutputHardwareBufferSize {
        device_id: String,
        target_frames: u32,
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
    GetAudioMetrics {
        response_tx: oneshot::Sender<AudioMetrics>,
    },
    StartRecording {
        session_id: String,
        recording_config: crate::audio::recording::RecordingConfig,
        response_tx: oneshot::Sender<Result<rtrb::Consumer<f32>>>,
    },
    StopRecording {
        session_id: String,
        response_tx: oneshot::Sender<Result<()>>,
    },
    StartIcecast {
        stream_id: String,
        config: crate::audio::broadcasting::StreamingServiceConfig,
        response_tx: oneshot::Sender<Result<rtrb::Consumer<f32>>>,
    },
    StopIcecast {
        stream_id: String,
        response_tx: oneshot::Sender<Result<()>>,
    },
}

/// Audio System Coordinator - lightweight interface between Tauri commands and audio pipeline
/// **NEW ARCHITECTURE**: AudioPipeline handles all audio processing, this just coordinates
pub struct IsolatedAudioManager {
    // **CORE**: 4-layer audio pipeline handles all audio processing
    audio_pipeline: AudioPipeline,

    // **HARDWARE**: StreamManager handles CoreAudio hardware streams
    stream_manager: StreamManager,

    // **RTRB BRIDGE**: Connect AudioPipeline outputs to devices (hardware + recording) via lock-free queues
    output_rtrb_producers: HashMap<String, Arc<Mutex<Producer<f32>>>>,

    // **COMMAND INTERFACE**: Handle Tauri audio commands
    command_rx: mpsc::Receiver<AudioCommand>,

    // **HARDWARE UPDATES**: Hardware buffer update commands from OutputWorker (macOS only)
    #[cfg(target_os = "macos")]
    hardware_update_rx: Option<mpsc::Receiver<AudioCommand>>,

    metrics: AudioMetrics,

    // **COORDINATION**: Event notifications for pipeline coordination
    global_input_notifier: Arc<Notify>,
    global_output_notifier: Arc<Notify>,
}

impl IsolatedAudioManager {
    /// **REMOVED**: Legacy resampler and VirtualMixer functionality
    /// AudioPipeline now handles all audio processing internally

    pub async fn new(command_rx: mpsc::Receiver<AudioCommand>, app_handle: Option<tauri::AppHandle>) -> Result<Self, anyhow::Error> {
        // **CORE**: Create 4-layer AudioPipeline with dynamic sample rate detection
        // Sample rate will be determined from the first device that gets added

        // **HARDWARE SYNC**: Create hardware update channel for CoreAudio buffer synchronization
        #[cfg(target_os = "macos")]
        let (hardware_update_tx, mut hardware_update_rx) = mpsc::channel::<AudioCommand>(32);

        #[cfg(target_os = "macos")]
        let audio_pipeline = if let Some(app_handle) = app_handle.clone() {
            // Create pipeline with both hardware updates and VU events
            let mut pipeline = AudioPipeline::new_with_app_handle(app_handle);
            pipeline.set_hardware_update_channel(hardware_update_tx);
            pipeline
        } else {
            AudioPipeline::new_with_hardware_updates(Some(hardware_update_tx))
        };

        #[cfg(not(target_os = "macos"))]
        let audio_pipeline = if let Some(app_handle) = app_handle.clone() {
            AudioPipeline::new_with_app_handle(app_handle)
        } else {
            AudioPipeline::new()
        };

        info!(
            "🎧 AUDIO_COORDINATOR: Initialized with 4-layer AudioPipeline (dynamic sample rate detection)"
        );

        Ok(Self {
            // **CORE**: Main audio processing pipeline
            audio_pipeline,

            // **HARDWARE**: CoreAudio stream management
            stream_manager: StreamManager::new(),

            // **BRIDGE**: SPMC queues for outputs (hardware + recording)
            output_rtrb_producers: HashMap::new(),

            // **INTERFACE**: Command handling
            command_rx,

            // **HARDWARE UPDATES**: Hardware buffer update receiver (macOS only)
            #[cfg(target_os = "macos")]
            hardware_update_rx: Some(hardware_update_rx),

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
            // **HARDWARE SYNC**: Create hardware update future conditionally (macOS only)
            #[cfg(target_os = "macos")]
            let hardware_future = async {
                if let Some(ref mut rx) = self.hardware_update_rx {
                    rx.recv().await
                } else {
                    std::future::pending().await
                }
            };

            #[cfg(not(target_os = "macos"))]
            let hardware_future = std::future::pending::<Option<AudioCommand>>();

            tokio::select! {
                // Handle Tauri audio commands
                command = self.command_rx.recv() => {
                    match command {
                        Some(cmd) => {
                            self.handle_command(cmd).await;
                        },
                        None => {
                            info!("🛑 {}: Command channel closed, shutting down", "AUDIO_COORDINATOR".red());
                            break;
                        }
                    }
                }

                // **HARDWARE SYNC**: Handle hardware buffer update requests from OutputWorker
                hardware_command = hardware_future => {
                    if let Some(cmd) = hardware_command {
                        match cmd {
                            AudioCommand::UpdateOutputHardwareBufferSize { device_id, target_frames } => {
                                info!(
                                    "🔄 {}: Processing hardware buffer update for {} → {} frames",
                                    "HARDWARE_SYNC".cyan(),
                                    device_id,
                                    target_frames
                                );
                                self.update_output_hardware_buffer_size(device_id, target_frames);
                            }
                            _ => {
                                warn!("⚠️ {}: Unexpected command on hardware channel: {:?}", "HARDWARE_SYNC".yellow(), std::mem::discriminant(&cmd));
                            }
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
            AudioCommand::SetAppHandle {
                app_handle,
                response_tx,
            } => {
                info!("{}: Received SetAppHandle command", "VU_COORDINATOR".bright_cyan());
                info!("{}: Setting AppHandle for VU level events", "VU_COORDINATOR".bright_cyan());
                self.audio_pipeline.set_app_handle(app_handle);
                info!("{}: AppHandle set successfully, sending confirmation", "VU_COORDINATOR".bright_cyan());
                let _ = response_tx.send(Ok(()));
            }
            AudioCommand::SetVUChannel {
                channel,
                response_tx,
            } => {
                info!("{}: Received SetVUChannel command", "VU_CHANNEL_COORD".bright_green());
                info!("{}: Setting VU channel for high-performance streaming", "VU_CHANNEL_COORD".bright_green());
                self.audio_pipeline.set_vu_channel(channel);
                info!("{}: VU channel set successfully, sending confirmation", "VU_CHANNEL_COORD".bright_green());
                let _ = response_tx.send(Ok(()));
            }
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
            AudioCommand::UpdateOutputHardwareBufferSize {
                device_id,
                target_frames,
            } => {
                self.update_output_hardware_buffer_size(device_id, target_frames);
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
            AudioCommand::GetAudioMetrics { response_tx } => {
                let metrics = self.get_metrics();
                let _ = response_tx.send(metrics);
            }
            AudioCommand::StartRecording {
                session_id,
                recording_config,
                response_tx,
            } => {
                let result = self
                    .handle_start_recording(session_id, recording_config)
                    .await;
                let _ = response_tx.send(result);
            }
            AudioCommand::StopRecording {
                session_id,
                response_tx,
            } => {
                let result = self.handle_stop_recording(session_id).await;
                let _ = response_tx.send(result);
            }
            AudioCommand::StartIcecast {
                stream_id,
                config,
                response_tx,
            } => {
                let result = self.handle_start_icecast(stream_id, config).await;
                let _ = response_tx.send(result);
            }
            AudioCommand::StopIcecast {
                stream_id,
                response_tx,
            } => {
                let result = self.handle_stop_icecast(stream_id).await;
                let _ = response_tx.send(result);
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
        // Check if input stream is already active by checking with the stream manager
        // Note: StreamManager tracks active CoreAudio input streams internally
        if self.stream_manager.has_input_stream(&device_id) {
            info!(
                "📋 {}: Input device '{}' already active, skipping duplicate creation",
                "DUPLICATE_INPUT_SKIP".yellow(),
                device_id
            );
            return Ok(());
        }

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
        let buffer_capacity = (native_sample_rate as usize * channels as usize) / 10; // 100ms for actual channel count
        let buffer_capacity = buffer_capacity.max(4096).min(16384);
        let (coreaudio_producer, audio_input_consumer) =
            rtrb::RingBuffer::<f32>::new(buffer_capacity);

        // **QUERY ACTUAL HARDWARE BUFFER SIZE**: Query hardware before creating streams
        let actual_buffer_frames =
            crate::audio::devices::coreaudio_stream::get_device_buffer_frame_size(
                coreaudio_device_id,
                false, // input device
            )
            .unwrap_or_else(|e| {
                warn!(
                    "⚠️ Failed to query input buffer size for {}: {}, using default 512",
                    device_id, e
                );
                512
            });

        // Convert frames to samples (frames × channels)
        let chunk_size = (actual_buffer_frames * channels as u32) as usize;

        info!(
            "🎯 {}: Input device '{}' - hardware: {} frames → {} samples ({} channels)",
            "CHUNK_SIZE_CALCULATION".green(),
            device_id,
            actual_buffer_frames,
            chunk_size,
            channels
        );

        // **PIPELINE INTEGRATION**: Create input worker FIRST to consume RTRB data
        let input_device_notifier = Arc::new(Notify::new());
        self.audio_pipeline.add_input_device_with_consumer(
            device_id.clone(),
            native_sample_rate,
            channels,
            chunk_size,
            audio_input_consumer,
            input_device_notifier.clone(),
        )?;

        // **HARDWARE STREAM**: Create CoreAudio stream AFTER pipeline worker is ready
        // Creating before will causes the queue to become full before starting and breaks audio processing.
        self.stream_manager.add_coreaudio_input_stream(
            device_id.clone(),
            coreaudio_device_id,
            device_name,
            channels,
            coreaudio_producer,
            input_device_notifier, // Use the same notifier for consistency
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
    /// Update hardware buffer size for a CoreAudio output stream
    #[cfg(target_os = "macos")]
    fn update_output_hardware_buffer_size(&mut self, device_id: String, target_frames: u32) {
        if let Err(e) = self
            .stream_manager
            .update_coreaudio_output_buffer_size(&device_id, target_frames)
        {
            tracing::warn!(
                "⚠️ Failed to update hardware buffer size for {}: {}",
                device_id,
                e
            );
        } else {
            tracing::info!(
                "🔄 {}: Updated hardware buffer size to {} frames for {}",
                "DYNAMIC_HARDWARE_SYNC".green(),
                target_frames,
                device_id
            );
        }
    }

    fn add_coreaudio_output_stream_direct(
        &mut self,
        device_id: String,
        coreaudio_device: crate::audio::types::CoreAudioDevice,
    ) -> Result<()> {
        // Check if output device is already active - prevent unnecessary stream restart
        if self.output_rtrb_producers.contains_key(&device_id) {
            info!(
                "📋 {}: Output device '{}' already active, skipping duplicate creation",
                "DUPLICATE_OUTPUT_SKIP".yellow(),
                device_id
            );
            return Ok(());
        }

        info!(
            "🔊 Creating CoreAudio output stream for device '{}' (ID: {})",
            device_id, coreaudio_device.device_id
        );

        // **ADAPTIVE AUDIO**: Detect actual device native sample rate like we do for inputs
        let native_sample_rate =
            crate::audio::devices::coreaudio_stream::get_device_native_sample_rate(
                coreaudio_device.device_id,
            )?;

        info!(
            "🔧 OUTPUT_DEVICE_RATE: Using detected {} Hz for output device '{}'",
            native_sample_rate, device_id
        );

        // Store device_id before moving coreaudio_device
        let coreaudio_device_id = coreaudio_device.device_id;

        // **QUERY ACTUAL HARDWARE BUFFER SIZE**: Query hardware before creating streams
        let actual_buffer_frames =
            crate::audio::devices::coreaudio_stream::get_device_buffer_frame_size(
                coreaudio_device_id,
                true, // output device
            )
            .unwrap_or_else(|e| {
                warn!(
                    "⚠️ Failed to query output buffer size for {}: {}, using default 512",
                    device_id, e
                );
                512
            });

        // **DYNAMIC CHANNEL DETECTION**: Get actual channel count from output device instead of assuming stereo
        let output_channels = coreaudio_device.channels; // Use actual channel count from device
        let chunk_size = (actual_buffer_frames * output_channels as u32) as usize;

        // **SIMPLIFIED QUEUE**: Create RTRB ring buffer for this output device - 4x the output chunk size
        // Since each output worker serves only one device, we don't need SPMC complexity
        let buffer_capacity = chunk_size * 4;

        let (rtrb_producer, rtrb_consumer) = rtrb::RingBuffer::<f32>::new(buffer_capacity);
        let rtrb_producer = Arc::new(Mutex::new(rtrb_producer));

        // **QUEUE TRACKING**: Create shared AtomicQueueTracker for this SPMC queue
        let queue_tracker =
            AtomicQueueTracker::new(format!("output_{}", device_id), buffer_capacity);

        // Store the RTRB producer for mixer to send audio data
        self.output_rtrb_producers
            .insert(device_id.clone(), rtrb_producer.clone());

        info!(
            "🎯 {}: Output device '{}' - hardware: {} frames → {} samples ({} channels)",
            "CHUNK_SIZE_CALCULATION".green(),
            device_id,
            actual_buffer_frames,
            chunk_size,
            output_channels
        );

        // **PIPELINE INTEGRATION**: Connect output device to AudioPipeline Layer 4 FIRST
        if let Err(e) = self
            .audio_pipeline
            .add_output_device_with_rtrb_producer_and_tracker(
                device_id.clone(),
                native_sample_rate,
                chunk_size,
                output_channels, // Pass the actual output device channel count
                Some(rtrb_producer.clone()),
                queue_tracker.clone(),
            )
        {
            error!(
                "❌ PIPELINE: Failed to connect output device '{}' to Layer 4: {}",
                device_id, e
            );
        } else {
            info!(
                "✅ PIPELINE: Connected output device '{}' to Layer 4 with SPMC writer at {} Hz (chunk: {} samples)",
                device_id, native_sample_rate, chunk_size
            );
        }

        // **HARDWARE STREAM**: Create CoreAudio stream AFTER pipeline worker is ready
        // Update the coreaudio_device to use the detected native sample rate
        let mut corrected_coreaudio_device = coreaudio_device;
        corrected_coreaudio_device.sample_rate = native_sample_rate;

        // Create the hardware CoreAudio stream with RTRB consumer using corrected sample rate
        self.stream_manager
            .add_coreaudio_output_stream_with_tracker(
                device_id.clone(),
                corrected_coreaudio_device,
                rtrb_consumer,
                self.global_output_notifier.clone(),
                queue_tracker.clone(),
            )?;

        self.metrics.output_streams = self.output_rtrb_producers.len();
        info!(
          "✅ CoreAudio output stream created and started for device '{}' with direct RTRB connection",
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


    fn get_metrics(&self) -> AudioMetrics {
        self.metrics.clone()
    }

    /// Start a recording session by creating an OutputWorker for recording
    async fn handle_start_recording(
        &mut self,
        session_id: String,
        recording_config: crate::audio::recording::RecordingConfig,
    ) -> Result<rtrb::Consumer<f32>> {
        info!(
            "🎙️ {}: Starting recording session '{}' with format: {}",
            "RECORDING_COORDINATOR".red(),
            session_id,
            recording_config.format.get_format_name()
        );

        // Check if recording already exists (only one recording allowed at a time)
        const RECORDING_DEVICE_ID: &str = "recording_output";
        if self.output_rtrb_producers.contains_key(RECORDING_DEVICE_ID) {
            return Err(anyhow::anyhow!(
                "Recording already active - only one recording allowed at a time"
            ));
        }

        // **RTRB SETUP**: Create buffer for AudioPipeline → Recording communication
        let buffer_capacity =
            (recording_config.sample_rate as usize * recording_config.channels as usize) / 10; // 100ms buffer
        let buffer_capacity = buffer_capacity.max(8192).min(32768); // Larger buffer for file I/O
        let (recording_producer, recording_consumer) =
            rtrb::RingBuffer::<f32>::new(buffer_capacity);

        info!(
            "🔧 {}: Created RTRB buffer with {} samples capacity for recording '{}'",
            "RECORDING_RTRB".red(),
            buffer_capacity,
            session_id
        );

        // **OUTPUT WORKER SETUP**: Create OutputWorker for recording (no hardware output)
        let queue_tracker = crate::audio::mixer::queue_manager::AtomicQueueTracker::new(
            RECORDING_DEVICE_ID.to_string(),
            buffer_capacity,
        );

        let producer_arc = Arc::new(tokio::sync::Mutex::new(recording_producer));

        // Add recording output worker to the audio pipeline
        let result = self
            .audio_pipeline
            .add_output_device_with_rtrb_producer_and_tracker(
                RECORDING_DEVICE_ID.to_string(),
                recording_config.sample_rate,
                1024, // Default chunk size for recording
                recording_config.channels,
                Some(producer_arc.clone()),
                queue_tracker,
            );

        match result {
            Ok(()) => {
                // Store the producer for cleanup later
                self.output_rtrb_producers
                    .insert(RECORDING_DEVICE_ID.to_string(), producer_arc);

                info!(
                    "✅ {}: Recording output worker created for session '{}'",
                    "RECORDING_COORDINATOR".red(),
                    session_id
                );

                // Return the consumer for the recording service
                Ok(recording_consumer)
            }
            Err(e) => {
                error!(
                    "❌ {}: Failed to create recording output worker for '{}': {}",
                    "RECORDING_ERROR".red(),
                    session_id,
                    e
                );
                Err(e)
            }
        }
    }

    /// Stop a recording session by removing the OutputWorker
    async fn handle_stop_recording(&mut self, _session_id: String) -> Result<()> {
        info!(
            "🛑 {}: Stopping recording session",
            "RECORDING_COORDINATOR".red()
        );

        const RECORDING_DEVICE_ID: &str = "recording_output";

        // Remove the OutputWorker from the audio pipeline
        // This will automatically clean up all resources (RTRB producer, worker thread, etc.)
        match self
            .audio_pipeline
            .remove_output_device(RECORDING_DEVICE_ID)
            .await
        {
            Ok(()) => {
                // Remove from our tracking as well
                self.output_rtrb_producers.remove(RECORDING_DEVICE_ID);

                info!(
                    "✅ {}: Recording OutputWorker stopped and removed from pipeline",
                    "RECORDING_COORDINATOR".red()
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "❌ {}: Failed to remove recording OutputWorker from pipeline: {}",
                    "RECORDING_ERROR".red(),
                    e
                );
                Err(e)
            }
        }
    }

    /// Start an Icecast streaming session by creating an OutputWorker for streaming
    async fn handle_start_icecast(
        &mut self,
        stream_id: String,
        config: crate::audio::broadcasting::StreamingServiceConfig,
    ) -> Result<rtrb::Consumer<f32>> {
        info!(
            "📡 {}: Starting Icecast stream '{}' to {}:{}{}",
            "ICECAST_COORDINATOR".blue(),
            stream_id,
            config.server_host,
            config.server_port,
            config.mount_point
        );

        // Use a unique device ID for each Icecast stream
        let icecast_device_id = format!("icecast_output_{}", stream_id);

        // Check if stream already exists
        if self.output_rtrb_producers.contains_key(&icecast_device_id) {
            return Err(anyhow::anyhow!(
                "Icecast stream '{}' is already running",
                stream_id
            ));
        }

        // **RTRB QUEUE**: Create ring buffer for audio data transport
        let buffer_size = 4096 * 16; // 16x larger buffer for network streaming
        let (streaming_producer, streaming_consumer) = rtrb::RingBuffer::<f32>::new(buffer_size);
        let producer_arc = Arc::new(Mutex::new(streaming_producer));

        // **QUEUE TRACKING**: Create queue tracker for dynamic sample rate adjustment
        let queue_tracker = AtomicQueueTracker::new(
            format!("icecast_{}", stream_id), // Unique queue ID
            buffer_size,                      // Capacity
        );

        // Add streaming output worker to the audio pipeline using the audio format from config
        let result = self
            .audio_pipeline
            .add_output_device_with_rtrb_producer_and_tracker(
                icecast_device_id.clone(),
                config.audio_format.sample_rate,
                1024, // Default chunk size for streaming
                config.audio_format.channels,
                Some(producer_arc.clone()),
                queue_tracker,
            );

        match result {
            Ok(()) => {
                // Store the producer for cleanup later
                self.output_rtrb_producers
                    .insert(icecast_device_id, producer_arc);

                info!(
                    "✅ {}: Icecast output worker created for stream '{}' ({}kbps, {}Hz)",
                    "ICECAST_COORDINATOR".blue(),
                    stream_id,
                    config.audio_format.bitrate,
                    config.audio_format.sample_rate
                );

                // Return the consumer for the Icecast service
                Ok(streaming_consumer)
            }
            Err(e) => {
                error!(
                    "❌ {}: Failed to create Icecast output worker for '{}': {}",
                    "ICECAST_ERROR".red(),
                    stream_id,
                    e
                );
                Err(e)
            }
        }
    }

    /// Stop an Icecast streaming session by removing the OutputWorker
    async fn handle_stop_icecast(&mut self, stream_id: String) -> Result<()> {
        info!(
            "🛑 {}: Stopping Icecast stream '{}'",
            "ICECAST_COORDINATOR".blue(),
            stream_id
        );

        let icecast_device_id = format!("icecast_output_{}", stream_id);

        // Remove the OutputWorker from the audio pipeline
        // This will automatically clean up all resources (RTRB producer, worker thread, etc.)
        match self
            .audio_pipeline
            .remove_output_device(&icecast_device_id)
            .await
        {
            Ok(()) => {
                // Remove from our tracking as well
                self.output_rtrb_producers.remove(&icecast_device_id);

                info!(
                    "✅ {}: Icecast OutputWorker stopped and removed from pipeline for stream '{}'",
                    "ICECAST_COORDINATOR".blue(),
                    stream_id
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "❌ {}: Failed to remove Icecast OutputWorker from pipeline for '{}': {}",
                    "ICECAST_ERROR".red(),
                    stream_id,
                    e
                );
                Err(e)
            }
        }
    }
}
