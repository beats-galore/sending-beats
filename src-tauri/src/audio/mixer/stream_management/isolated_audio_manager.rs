use anyhow::{Context, Result};
use colored::*;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use super::virtual_mixer::VirtualMixer;
use crate::audio::effects::{CustomAudioEffectsChain, EQBand};
use crate::audio::mixer::AudioPipeline;
use crate::audio::types::AudioChannel;
use tokio::sync::{mpsc, oneshot, Mutex, Notify};

// Internal stream_management module imports
use super::stream_manager::{AudioMetrics, StreamManager};
use crate::audio::devices::coreaudio_stream::CoreAudioInputStream;

// Lock-free audio buffer imports
use crate::audio::mixer::latency_probe::{LatencySnapshot, LatencyStage};
use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use rtrb::{Consumer, Producer, RingBuffer};

// Command channel for isolated audio thread communication
// Cannot derive Debug because Device doesn't implement Debug
pub enum AudioCommand {
    SetVUChannel {
        channel: tauri::ipc::Channel<crate::audio::VUChannelData>,
        response_tx: oneshot::Sender<Result<()>>,
    },
    RemoveInputStream {
        device_id: String,
        response_tx: oneshot::Sender<Result<bool>>,
    },
    RemoveOutputStream {
        device_id: String,
        response_tx: oneshot::Sender<Result<()>>,
    },
    /// Tear down every device belonging to the current session, so a new session
    /// can register its own devices from a clean slate
    ClearSessionDevices {
        response_tx: oneshot::Sender<Result<usize>>,
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
        response_tx: oneshot::Sender<Result<()>>,
    },
    #[cfg(target_os = "macos")]
    AddApplicationAudioInputStream {
        device_id: String,
        pid: u32,
        device_name: String,
        channels: u16,
        producer: Producer<f32>,
        response_tx: oneshot::Sender<Result<()>>,
    },
    /// Attach a file player's queue as an input
    ///
    /// Unlike the two above there is no hardware to open: the player is handed
    /// over whole and a decoding thread reads it at the rate the mixer drains.
    AddFilePlayerInputStream {
        device_id: String,
        player: std::sync::Arc<crate::audio::file_player::AudioFilePlayer>,
        device_name: String,
        sample_rate: u32,
        channels: u16,
        response_tx: oneshot::Sender<Result<()>>,
    },
    UpdateEffects {
        device_id: String,
        effects: CustomAudioEffectsChain,
        response_tx: oneshot::Sender<Result<()>>,
    },
    GetAudioMetrics {
        response_tx: oneshot::Sender<AudioMetrics>,
    },
    /// Current per-stage latency of the running pipeline
    GetLatencySnapshot {
        response_tx: oneshot::Sender<LatencySnapshot>,
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
    UpdateInputGain {
        device_id: String,
        gain: f32,
        response_tx: oneshot::Sender<Result<()>>,
    },
    UpdateInputPan {
        device_id: String,
        pan: f32,
        response_tx: oneshot::Sender<Result<()>>,
    },
    UpdateInputEffectsEnabled {
        device_id: String,
        enabled: bool,
        response_tx: oneshot::Sender<Result<()>>,
    },
    UpdateInputMuted {
        device_id: String,
        muted: bool,
        response_tx: oneshot::Sender<Result<()>>,
    },
    UpdateInputSolo {
        device_id: String,
        solo: bool,
        response_tx: oneshot::Sender<Result<()>>,
    },
    UpdateMasterGain {
        gain: f32,
        response_tx: oneshot::Sender<Result<()>>,
    },
    /// Which inputs feed which mix, and which outputs take it
    Bus(BusCommand),
}

mod bus_commands;
pub use bus_commands::BusCommand;

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

    // **FILE PLAYERS**: Decoding threads feeding queued files into the pipeline.
    // Held here because dropping one stops its thread, so a source outlives the
    // command that made it and no longer than the device it belongs to.
    file_player_sources: HashMap<String, crate::audio::file_player::FilePlayerSource>,

    database: Option<Arc<crate::db::AudioDatabase>>,
}

impl IsolatedAudioManager {
    pub async fn new(
        command_rx: mpsc::Receiver<AudioCommand>,
        database: Option<Arc<crate::db::AudioDatabase>>,
    ) -> Result<Self, anyhow::Error> {
        // **CORE**: Create 4-layer AudioPipeline with dynamic sample rate detection
        // Sample rate will be determined from the first device that gets added

        // **HARDWARE SYNC**: Create hardware update channel for CoreAudio buffer synchronization
        #[cfg(target_os = "macos")]
        let (hardware_update_tx, mut hardware_update_rx) = mpsc::channel::<AudioCommand>(32);

        #[cfg(target_os = "macos")]
        let audio_pipeline = AudioPipeline::new_with_hardware_updates(Some(hardware_update_tx));

        #[cfg(not(target_os = "macos"))]
        let audio_pipeline = AudioPipeline::new();

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

            // **FILE PLAYERS**: Decoding threads, started as players are attached
            file_player_sources: HashMap::new(),

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

            database,
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
                            info!("🛑 {}: Command channel closed, shutting down", "AUDIO_COORDINATOR".on_yellow().red());
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
                                    "HARDWARE_SYNC".on_yellow().red(),
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
            AudioCommand::SetVUChannel {
                channel,
                response_tx,
            } => {
                info!(
                    "{}: Received SetVUChannel command",
                    "VU_CHANNEL_COORD".on_yellow().red()
                );
                info!(
                    "{}: Setting VU channel for high-performance streaming",
                    "VU_CHANNEL_COORD".on_yellow().red()
                );
                self.audio_pipeline.set_vu_channel(channel);
                info!(
                    "{}: VU channel set successfully, sending confirmation",
                    "VU_CHANNEL_COORD".on_yellow().red()
                );
                let _ = response_tx.send(Ok(()));
            }
            AudioCommand::RemoveInputStream {
                device_id,
                response_tx,
            } => {
                let result = self.handle_remove_input_stream(device_id).await;
                let _ = response_tx.send(Ok(result));
            }
            AudioCommand::RemoveOutputStream {
                device_id,
                response_tx,
            } => {
                let result = self.handle_remove_output_stream(device_id).await;
                let _ = response_tx.send(result);
            }
            AudioCommand::ClearSessionDevices { response_tx } => {
                let result = self.handle_clear_session_devices().await;
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
                response_tx,
            } => {
                let result = self
                    .handle_add_coreaudio_input_stream(
                        device_id,
                        coreaudio_device_id,
                        device_name,
                        channels,
                        producer,
                    )
                    .await;
                let _ = response_tx.send(result);
            }
            #[cfg(target_os = "macos")]
            AudioCommand::AddApplicationAudioInputStream {
                device_id,
                pid,
                device_name,
                channels,
                producer,
                response_tx,
            } => {
                let result = self
                    .handle_add_application_audio_input_stream(
                        device_id,
                        pid,
                        device_name,
                        channels,
                        producer,
                    )
                    .await;
                let _ = response_tx.send(result);
            }
            AudioCommand::AddFilePlayerInputStream {
                device_id,
                player,
                device_name,
                sample_rate,
                channels,
                response_tx,
            } => {
                let result = self
                    .handle_add_file_player_input_stream(
                        device_id,
                        player,
                        device_name,
                        sample_rate,
                        channels,
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
            AudioCommand::GetLatencySnapshot { response_tx } => {
                let _ = response_tx.send(self.audio_pipeline.latency_snapshot());
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
            AudioCommand::UpdateInputGain {
                device_id,
                gain,
                response_tx,
            } => {
                let result = self.audio_pipeline.update_input_gain(&device_id, gain);
                let _ = response_tx.send(result);
            }
            AudioCommand::UpdateInputPan {
                device_id,
                pan,
                response_tx,
            } => {
                let result = self.audio_pipeline.update_input_pan(&device_id, pan);
                let _ = response_tx.send(result);
            }
            AudioCommand::UpdateInputEffectsEnabled {
                device_id,
                enabled,
                response_tx,
            } => {
                let result = self
                    .audio_pipeline
                    .update_input_effects_enabled(&device_id, enabled);
                let _ = response_tx.send(result);
            }
            AudioCommand::UpdateInputMuted {
                device_id,
                muted,
                response_tx,
            } => {
                let result = self.audio_pipeline.update_input_muted(&device_id, muted);
                let _ = response_tx.send(result);
            }
            AudioCommand::UpdateInputSolo {
                device_id,
                solo,
                response_tx,
            } => {
                let result = self.audio_pipeline.update_input_solo(&device_id, solo);
                let _ = response_tx.send(result);
            }
            AudioCommand::UpdateMasterGain { gain, response_tx } => {
                let result = self.audio_pipeline.update_master_gain(gain);
                let _ = response_tx.send(result);
            }
            AudioCommand::Bus(command) => self.handle_bus_command(command),
        }
    }

    /// Publish what a device's own hardware costs on the latency path
    ///
    /// Fixed for the life of the stream: the buffer CoreAudio hands over each
    /// callback, plus the presentation latency and safety offset the device
    /// reports. Nothing in the pipeline can reduce it.
    #[cfg(target_os = "macos")]
    fn record_hardware_latency(
        &self,
        device_id: &str,
        coreaudio_device_id: coreaudio_sys::AudioDeviceID,
        buffer_frames: u32,
        sample_rate: u32,
        is_output: bool,
    ) {
        let extra_frames = crate::audio::devices::coreaudio_stream::get_device_extra_latency_frames(
            coreaudio_device_id,
            is_output,
        );

        let stage = if is_output {
            LatencyStage::OutputHardware
        } else {
            LatencyStage::InputHardware
        };

        self.audio_pipeline
            .latency_probe()
            .gauge(device_id, stage)
            .set_frames((buffer_frames + extra_frames) as usize, sample_rate);
    }

    #[cfg(target_os = "macos")]
    async fn handle_add_coreaudio_input_stream(
        &mut self,
        device_id: String,
        coreaudio_device_id: coreaudio_sys::AudioDeviceID,
        device_name: String,
        channels: u16,
        producer: Producer<f32>,
    ) -> Result<()> {
        info!(
            "🔍 HANDLE_ADD_INPUT: device_id='{}', coreaudio_device_id={}, device_name='{}', channels={}",
            device_id, coreaudio_device_id, device_name, channels
        );

        // Check if input stream is already active by checking with the stream manager
        // Note: StreamManager tracks active CoreAudio input streams internally
        if self.stream_manager.has_input_stream(&device_id) {
            info!(
                "📋 {}: Input device '{}' already active, skipping duplicate creation",
                "DUPLICATE_INPUT_SKIP".on_yellow().red(),
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
                &device_id,
            )?;

        // **RTRB SETUP**: Create buffer for hardware → AudioPipeline communication
        let buffer_capacity = (native_sample_rate as usize * channels as usize) / 10; // 100ms for actual channel count
        let buffer_capacity = buffer_capacity.max(4096).min(96000);
        let (coreaudio_producer, audio_input_consumer) =
            rtrb::RingBuffer::<f32>::new(buffer_capacity);

        // **NEGOTIATE HARDWARE BUFFER SIZE**: ask for a small one before the stream
        // exists, since a device already in use will not resize
        let actual_buffer_frames =
            crate::audio::devices::coreaudio_stream::negotiate_device_buffer_frame_size(
                coreaudio_device_id,
                &device_id,
                false, // input device
            );

        // Convert frames to samples (frames × channels)
        let chunk_size = (actual_buffer_frames * channels as u32) as usize;

        info!(
            "🎯 {}: Input device '{}' - hardware: {} frames → {} samples ({} channels)",
            "CHUNK_SIZE_CALCULATION".on_yellow().red(),
            device_id,
            actual_buffer_frames,
            chunk_size,
            channels
        );

        let channel_number = if let Some(ref db) = self.database {
            match crate::db::ConfiguredAudioDeviceService::get_channel_number_for_active_device(
                db.sea_orm(),
                &device_id,
            )
            .await
            {
                Ok(Some(channel)) => {
                    info!(
                        "🎯 {}: Found channel number {} for device '{}'",
                        "CHANNEL_LOOKUP".on_yellow().red(),
                        channel,
                        device_id
                    );
                    channel
                }
                Ok(None) => {
                    return Err(anyhow::anyhow!(
                        "No channel configuration found for device '{}' in active session",
                        device_id
                    ));
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Database error getting channel number for device '{}': {}",
                        device_id,
                        e
                    ));
                }
            }
        } else {
            return Err(anyhow::anyhow!(
                "No database available to lookup channel number for device '{}'",
                device_id
            ));
        };

        // Load initial audio effects from database
        let (initial_gain, initial_pan, initial_muted, initial_solo) = if let Some(ref db) =
            self.database
        {
            match crate::db::AudioEffectsDefaultService::find_by_device_identifier_in_active_config(
                db.sea_orm(),
                &device_id,
            )
            .await
            {
                Ok(Some(effects)) => {
                    info!(
                        "🎛️ {}: Loaded initial effects for '{}': gain={}, pan={}, muted={}, solo={}",
                        "EFFECTS_LOAD".on_yellow().red(),
                        device_id,
                        effects.gain,
                        effects.pan,
                        effects.muted,
                        effects.solo
                    );
                    (
                        Some(effects.gain),
                        Some(effects.pan),
                        Some(effects.muted),
                        Some(effects.solo),
                    )
                }
                Ok(None) => {
                    info!(
                        "ℹ️ {}: No saved effects found for '{}', using defaults",
                        "EFFECTS_LOAD".on_yellow().red(),
                        device_id
                    );
                    (None, None, None, None)
                }
                Err(e) => {
                    warn!(
                        "⚠️ {}: Failed to load effects for '{}': {}, using defaults",
                        "EFFECTS_LOAD".on_yellow().red(),
                        device_id,
                        e
                    );
                    (None, None, None, None)
                }
            }
        } else {
            (None, None, None, None)
        };

        // **PIPELINE INTEGRATION**: Create input worker FIRST to consume RTRB data
        self.audio_pipeline
            .add_input_device_with_consumer_and_producer(
                device_id.clone(),
                native_sample_rate,
                channels,
                chunk_size,
                audio_input_consumer,
                channel_number,
                initial_gain,
                initial_pan,
                initial_muted,
                initial_solo,
            )?;

        self.record_hardware_latency(
            &device_id,
            coreaudio_device_id,
            actual_buffer_frames,
            native_sample_rate,
            false,
        );

        // **HARDWARE STREAM**: Create CoreAudio stream AFTER pipeline worker is ready
        // Creating before will causes the queue to become full before starting and breaks audio processing.
        self.stream_manager.add_coreaudio_input_stream(
            device_id.clone(),
            coreaudio_device_id,
            device_name,
            channels,
            coreaudio_producer,
        )?;

        info!(
            "✅ {}: Input device '{}' connected to AudioPipeline",
            "AUDIO_COORDINATOR".on_yellow().red(),
            device_id
        );
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn handle_add_application_audio_input_stream(
        &mut self,
        device_id: String,
        pid: u32,
        device_name: String,
        channels: u16,
        _producer: Producer<f32>,
    ) -> Result<()> {
        if self.stream_manager.has_input_stream(&device_id) {
            info!(
                "📋 {}: Application audio device '{}' already active, skipping duplicate creation",
                "DUPLICATE_APP_AUDIO_SKIP".on_yellow().red(),
                device_id
            );
            return Ok(());
        }

        info!(
            "🎯 AUDIO_COORDINATOR: Adding application audio input stream for device '{}' (PID: {})",
            device_id, pid
        );

        // **SCREENCAPTUREKIT**: Create ScreenCaptureKit stream and detect sample rate
        // Create RTRB ring buffer for ScreenCaptureKit → pipeline communication
        let initial_buffer_capacity = 96000;
        let (screencapture_producer, audio_input_consumer) =
            rtrb::RingBuffer::<f32>::new(initial_buffer_capacity);

        // Create ScreenCaptureKit stream and start capture (returns detected sample rate)
        let detected_sample_rate = self.stream_manager.add_screencapture_stream(
            device_id.clone(),
            pid as i32,
            device_name.clone(),
            screencapture_producer,
        )?;

        let native_sample_rate = detected_sample_rate as u32;

        info!(
            "🎯 {}: Detected sample rate {} Hz for application audio device '{}'",
            "APP_AUDIO_SAMPLE_RATE".on_yellow().red(),
            native_sample_rate,
            device_id
        );

        // Calculate buffer capacity based on detected sample rate
        let buffer_capacity = (native_sample_rate as usize * channels as usize) / 10;
        let buffer_capacity = buffer_capacity.max(4096).min(96000);

        // Use 512 frames as chunk size (will be multiplied by channels in pipeline)
        let chunk_size = 512 * channels as usize;

        let channel_number = if let Some(ref db) = self.database {
            match crate::db::ConfiguredAudioDeviceService::get_channel_number_for_active_device(
                db.sea_orm(),
                &device_id,
            )
            .await
            {
                Ok(Some(channel)) => {
                    info!(
                        "🎯 {}: Found channel number {} for application audio device '{}'",
                        "CHANNEL_LOOKUP".on_yellow().red(),
                        channel,
                        device_id
                    );
                    channel
                }
                Ok(None) => {
                    return Err(anyhow::anyhow!(
                        "No channel configuration found for device '{}' in active session",
                        device_id
                    ));
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Database error getting channel number for device '{}': {}",
                        device_id,
                        e
                    ));
                }
            }
        } else {
            return Err(anyhow::anyhow!(
                "No database available to lookup channel number for device '{}'",
                device_id
            ));
        };

        let (initial_gain, initial_pan, initial_muted, initial_solo) = if let Some(ref db) =
            self.database
        {
            match crate::db::AudioEffectsDefaultService::find_by_device_identifier_in_active_config(
                db.sea_orm(),
                &device_id,
            )
            .await
            {
                Ok(Some(effects)) => {
                    info!(
                        "🎛️ {}: Loaded initial effects for '{}': gain={}, pan={}, muted={}, solo={}",
                        "EFFECTS_LOAD".on_yellow().red(),
                        device_id,
                        effects.gain,
                        effects.pan,
                        effects.muted,
                        effects.solo
                    );
                    (
                        Some(effects.gain),
                        Some(effects.pan),
                        Some(effects.muted),
                        Some(effects.solo),
                    )
                }
                Ok(None) => (None, None, None, None),
                Err(e) => {
                    warn!(
                        "⚠️ {}: Failed to load effects for '{}': {}, using defaults",
                        "EFFECTS_LOAD".on_yellow().red(),
                        device_id,
                        e
                    );
                    (None, None, None, None)
                }
            }
        } else {
            (None, None, None, None)
        };

        self.audio_pipeline
            .add_input_device_with_consumer_and_producer(
                device_id.clone(),
                native_sample_rate,
                channels,
                chunk_size,
                audio_input_consumer,
                channel_number,
                initial_gain,
                initial_pan,
                initial_muted,
                initial_solo,
            )?;

        info!(
            "✅ {}: Application audio device '{}' connected to AudioPipeline via ScreenCaptureKit",
            "AUDIO_COORDINATOR".on_yellow().red(),
            device_id
        );

        Ok(())
    }

    /// Attach a file player's queue as an input device
    ///
    /// There is no hardware to open and no rate to detect: the player declares
    /// what it emits and brings every file in its queue to that, so this only
    /// has to build the queue between the decoder and the pipeline and start the
    /// thread that fills it.
    async fn handle_add_file_player_input_stream(
        &mut self,
        device_id: String,
        player: Arc<crate::audio::file_player::AudioFilePlayer>,
        device_name: String,
        sample_rate: u32,
        channels: u16,
    ) -> Result<()> {
        if self.file_player_sources.contains_key(&device_id) {
            info!(
                "📋 {}: File player '{}' already attached, skipping duplicate creation",
                "DUPLICATE_FILE_PLAYER_SKIP".on_magenta().white(),
                device_id
            );
            return Ok(());
        }

        info!(
            "🎼 {}: Attaching file player '{}' ({} Hz, {} ch)",
            "AUDIO_COORDINATOR".on_yellow().red(),
            device_name,
            sample_rate,
            channels
        );

        let chunk_size = crate::audio::file_player::SOURCE_CHUNK_FRAMES * channels as usize;

        // Room for several chunks so a late wake-up on either side is absorbed
        // rather than heard.
        let buffer_capacity = chunk_size * 16;
        let (producer, consumer) = rtrb::RingBuffer::<f32>::new(buffer_capacity);

        let (channel_number, initial_gain, initial_pan, initial_muted, initial_solo) =
            self.channel_placement_for(&device_id).await?;

        // Started before registering, so the queue already has audio in it by
        // the time the input worker's first read comes round.
        let source =
            crate::audio::file_player::FilePlayerSource::start(device_id.clone(), player, producer);
        self.file_player_sources.insert(device_id.clone(), source);

        if let Err(e) = self
            .audio_pipeline
            .add_input_device_with_consumer_and_producer(
                device_id.clone(),
                sample_rate,
                channels,
                chunk_size,
                consumer,
                channel_number,
                initial_gain,
                initial_pan,
                initial_muted,
                initial_solo,
            )
        {
            // The thread is stopped rather than left decoding into a queue
            // nothing will ever read.
            self.file_player_sources.remove(&device_id);
            return Err(e);
        }

        info!(
            "✅ {}: File player '{}' connected to AudioPipeline",
            "AUDIO_COORDINATOR".on_yellow().red(),
            device_id
        );

        Ok(())
    }

    /// Where a device sits in the session, and how it was last left
    ///
    /// The pipeline needs the channel number while building the device, so this
    /// has to be read before the stream is registered rather than after.
    async fn channel_placement_for(
        &self,
        device_id: &str,
    ) -> Result<(u32, Option<f32>, Option<f32>, Option<bool>, Option<bool>)> {
        let Some(ref db) = self.database else {
            return Err(anyhow::anyhow!(
                "No database available to look up the channel for '{}'",
                device_id
            ));
        };

        let channel_number =
            crate::db::ConfiguredAudioDeviceService::get_channel_number_for_active_device(
                db.sea_orm(),
                device_id,
            )
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No channel configuration found for '{}' in the active session",
                    device_id
                )
            })?;

        // Absent effects are not a failure: a device patched for the first time
        // has none, and the pipeline takes its own defaults for each.
        let effects =
            crate::db::AudioEffectsDefaultService::find_by_device_identifier_in_active_config(
                db.sea_orm(),
                device_id,
            )
            .await
            .ok()
            .flatten();

        Ok(match effects {
            Some(effects) => (
                channel_number,
                Some(effects.gain),
                Some(effects.pan),
                Some(effects.muted),
                Some(effects.solo),
            ),
            None => (channel_number, None, None, None, None),
        })
    }

    async fn handle_remove_input_stream(&mut self, device_id: String) -> bool {
        info!(
            "🗑️ {}: Removing input device '{}' (called from safe_switch_input_device)",
            "AUDIO_COORDINATOR".on_yellow().red(),
            device_id
        );
        info!("🔍 HANDLE_REMOVE_INPUT: device_id='{}'", device_id);

        // **PIPELINE**: Remove device from AudioPipeline
        if let Err(e) = self.audio_pipeline.remove_input_device(&device_id).await {
            warn!("⚠️ Failed to remove input device from pipeline: {}", e);
        }

        // **FILE PLAYER**: Dropping the source stops its decoding thread. Left
        // behind it would keep decoding into a queue nothing reads.
        if self.file_player_sources.remove(&device_id).is_some() {
            info!(
                "🎼 {}: Stopped decoding for '{}'",
                "AUDIO_COORDINATOR".on_yellow().red(),
                device_id
            );
        }

        // **HARDWARE**: Remove hardware stream
        self.stream_manager.remove_stream(&device_id);

        info!(
            "✅ {}: Removed input device '{}'",
            "AUDIO_COORDINATOR".on_yellow().red(),
            device_id
        );
        true
    }

    /// Tear down an output device so its slot can be given to another one.
    ///
    /// Refuses internal taps: recording and Icecast register as outputs to
    /// receive the mix, and removing one from here would silently stop a
    /// broadcast that the user never asked to end.
    async fn handle_remove_output_stream(&mut self, device_id: String) -> Result<()> {
        info!(
            "🗑️ {}: Removing output device '{}'",
            "AUDIO_COORDINATOR".on_yellow().red(),
            device_id
        );

        if Self::is_internal_output(&device_id) {
            return Err(anyhow::anyhow!(
                "'{}' is an internal output and cannot be removed",
                device_id
            ));
        }

        self.audio_pipeline.remove_output_device(&device_id).await?;

        // **HARDWARE**: Remove hardware stream
        self.stream_manager.remove_stream(&device_id);

        info!(
            "✅ {}: Removed output device '{}'",
            "AUDIO_COORDINATOR".on_yellow().red(),
            device_id
        );
        Ok(())
    }

    /// Device IDs that are internal taps rather than session devices
    ///
    /// Recording and Icecast register themselves as outputs to receive the mix,
    /// so a session teardown must leave them running.
    fn is_internal_output(device_id: &str) -> bool {
        device_id == "recording_output" || device_id.starts_with("icecast_output_")
    }

    /// Remove every session device so a new session can register from scratch
    ///
    /// Returns the number of devices removed.
    async fn handle_clear_session_devices(&mut self) -> usize {
        let input_ids = self.audio_pipeline.input_device_ids();
        let output_ids: Vec<String> = self
            .audio_pipeline
            .output_device_ids()
            .into_iter()
            .filter(|id| !Self::is_internal_output(id))
            .collect();

        info!(
            "🧹 {}: Clearing session devices ({} inputs, {} outputs)",
            "AUDIO_COORDINATOR".on_yellow().red(),
            input_ids.len(),
            output_ids.len()
        );

        let mut removed = 0;

        for device_id in input_ids {
            if self.handle_remove_input_stream(device_id).await {
                removed += 1;
            }
        }

        for device_id in output_ids {
            if let Err(e) = self.audio_pipeline.remove_output_device(&device_id).await {
                warn!(
                    "⚠️ Failed to remove output device '{}' from pipeline: {}",
                    device_id, e
                );
                continue;
            }

            self.stream_manager.remove_stream(&device_id);
            self.output_rtrb_producers.remove(&device_id);
            removed += 1;

            info!(
                "🗑️ {}: Removed output device '{}'",
                "AUDIO_COORDINATOR".on_yellow().red(),
                device_id
            );
        }

        self.metrics.output_streams = self.output_rtrb_producers.len();

        info!(
            "✅ {}: Cleared {} session devices",
            "AUDIO_COORDINATOR".on_yellow().red(),
            removed
        );

        removed
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
                "DYNAMIC_HARDWARE_SYNC".on_yellow().red(),
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
        // Check if output device is already active - prevent unnecessary stream restart.
        // Both books have to be consulted: this coordinator tracks outputs by RTRB
        // producer while the pipeline tracks them by worker, and a device present in
        // only one of them would otherwise be rejected as a duplicate registration
        // instead of being recognised as already connected.
        if self.output_rtrb_producers.contains_key(&device_id)
            || self.audio_pipeline.has_output_device(&device_id)
        {
            info!(
                "📋 {}: Output device '{}' already active, skipping duplicate creation",
                "DUPLICATE_OUTPUT_SKIP".on_yellow().red(),
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
                &device_id,
            )?;

        info!(
            "🔧 {}: Using detected {} Hz for output device '{}'",
            "OUTPUT_DEVICE_RATE".on_yellow().red(),
            native_sample_rate,
            device_id
        );

        // Store device_id before moving coreaudio_device
        let coreaudio_device_id = coreaudio_device.device_id;

        // **NEGOTIATE HARDWARE BUFFER SIZE**: ask for a small one before the stream
        // exists, since a device already in use will not resize
        let actual_buffer_frames =
            crate::audio::devices::coreaudio_stream::negotiate_device_buffer_frame_size(
                coreaudio_device_id,
                &device_id,
                true, // output device
            );

        // **DYNAMIC CHANNEL DETECTION**: Get actual channel count from output device instead of assuming stereo
        let output_channels = coreaudio_device.channels; // Use actual channel count from device
        let chunk_size = (actual_buffer_frames * output_channels as u32) as usize;

        // **SIMPLIFIED QUEUE**: Create RTRB ring buffer for this output device - 4x the output chunk size
        // Producer goes to OutputWorker (writes), Consumer goes to CoreAudio (reads)
        // Floored on time, not on chunks. Sized in chunks it shrinks with the
        // hardware buffer, and at small buffers the ring ends up too small to hold
        // the cushion the worker is trying to keep.
        let buffer_capacity = (chunk_size * 4).max(
            crate::audio::mixer::pipeline::pacing::jitter_cushion_samples(
                native_sample_rate,
                output_channels,
            ) * 4,
        );

        let (rtrb_producer, rtrb_consumer) = rtrb::RingBuffer::<f32>::new(buffer_capacity);

        // NOTE: Producer is NOT wrapped in Arc<Mutex<>> - OutputWorker will own it directly
        // Consumer goes to CoreAudio stream (also owned directly)

        // **QUEUE TRACKING**: Create AtomicQueueTracker for monitoring this queue
        // What the output worker paces this ring to, so the drift controller and
        // the pacing are steering towards the same level rather than against it.
        let queue_tracker =
            AtomicQueueTracker::new(format!("output_{}", device_id), buffer_capacity)
                .with_target_fill(
                    crate::audio::mixer::pipeline::audio_worker::target_downstream_samples(
                        chunk_size,
                        native_sample_rate,
                        output_channels,
                    ),
                );

        info!(
            "🎯 {}: Output device '{}' - hardware: {} frames → {} samples ({} channels)",
            "CHUNK_SIZE_CALCULATION".on_yellow().red(),
            device_id,
            actual_buffer_frames,
            chunk_size,
            output_channels
        );

        // **PIPELINE INTEGRATION**: Connect output device to AudioPipeline Layer 4 FIRST
        //
        // This must abort on failure. The producer above is moved into the call and
        // dropped with the error, so continuing would hand the hardware stream a
        // consumer nothing writes to, while the already-registered OutputWorker keeps
        // filling a buffer nothing reads. Both halves go quiet and the device appears
        // to be working because the level meters read from the mixing layer.
        self.audio_pipeline
            .add_output_device_with_rtrb_producer_and_tracker(
                device_id.clone(),
                native_sample_rate,
                chunk_size,
                output_channels,     // Pass the actual output device channel count
                Some(rtrb_producer), // Pass raw producer - OutputWorker will own it
                queue_tracker.clone(),
            )
            .inspect_err(|e| {
                error!(
                    "❌ PIPELINE: Failed to connect output device '{}' to Layer 4: {}",
                    device_id, e
                );
            })?;

        info!(
            "✅ PIPELINE: Connected output device '{}' to Layer 4 with SPMC writer at {} Hz (chunk: {} samples)",
            device_id, native_sample_rate, chunk_size
        );

        self.record_hardware_latency(
            &device_id,
            coreaudio_device_id,
            actual_buffer_frames,
            native_sample_rate,
            true,
        );

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
                queue_tracker.clone(),
            )?;

        self.metrics.output_streams = self.output_rtrb_producers.len();
        info!(
            "✅ {}: output stream created and started for device '{}' with direct RTRB connection",
            "ISOLATED_AUDIO_MANAGER_CORE_AUDIO_OUTPUT".on_yellow().red(),
            device_id
        );
        Ok(())
    }

    fn handle_update_effects(
        &mut self,
        device_id: String,
        effects: CustomAudioEffectsChain,
    ) -> Result<()> {
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
            "RECORDING_COORDINATOR".bright_green(),
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
        let buffer_capacity = buffer_capacity.max(96000).min(384000); // Larger buffer for file I/O
        let (recording_producer, recording_consumer) =
            rtrb::RingBuffer::<f32>::new(buffer_capacity);

        info!(
            "🔧 {}: Created RTRB buffer with {} samples capacity for recording '{}'",
            "RECORDING_RTRB".on_yellow().red(),
            buffer_capacity,
            session_id
        );

        // **OUTPUT WORKER SETUP**: Create OutputWorker for recording (no hardware output)
        let queue_tracker = crate::audio::mixer::queue_manager::AtomicQueueTracker::new(
            RECORDING_DEVICE_ID.to_string(),
            buffer_capacity,
        );

        let result = self
            .audio_pipeline
            .add_output_device_with_rtrb_producer_and_tracker(
                RECORDING_DEVICE_ID.to_string(),
                recording_config.sample_rate,
                1024,
                recording_config.channels,
                Some(recording_producer),
                queue_tracker,
            );

        match result {
            Ok(()) => {
                info!(
                    "✅ {}: Recording output worker created for session '{}'",
                    "RECORDING_COORDINATOR".bright_green(),
                    session_id
                );

                // Return the consumer for the recording service
                Ok(recording_consumer)
            }
            Err(e) => {
                error!(
                    "❌ {}: Failed to create recording output worker for '{}': {}",
                    "RECORDING_ERROR".bright_green(),
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
            "RECORDING_COORDINATOR".bright_green()
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
                    "RECORDING_COORDINATOR".bright_green()
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "❌ {}: Failed to remove recording OutputWorker from pipeline: {}",
                    "RECORDING_ERROR".on_yellow().red(),
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
        let buffer_size = 4096 * 16;
        let (streaming_producer, streaming_consumer) = rtrb::RingBuffer::<f32>::new(buffer_size);

        let queue_tracker = AtomicQueueTracker::new(format!("icecast_{}", stream_id), buffer_size);

        let result = self
            .audio_pipeline
            .add_output_device_with_rtrb_producer_and_tracker(
                icecast_device_id.clone(),
                config.audio_format.sample_rate,
                1024,
                config.audio_format.channels,
                Some(streaming_producer),
                queue_tracker,
            );

        match result {
            Ok(()) => {
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
                    "ICECAST_ERROR".blue(),
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
                    "ICECAST_ERROR".blue(),
                    stream_id,
                    e
                );
                Err(e)
            }
        }
    }
}
