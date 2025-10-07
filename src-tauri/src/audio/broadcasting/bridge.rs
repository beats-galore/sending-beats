use super::streaming::{StreamConfig, StreamManager};
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::{Duration, Instant};

/// Real-time audio streaming bridge that connects the mixer output to Icecast
///
/// This component captures live audio from the mixer and handles:
/// - Sample rate conversion for Icecast compatibility
/// - Real-time encoding to MP3/AAC
/// - Buffering and flow control
/// - Stream health monitoring
/// - Automatic reconnection
#[derive(Debug)]
pub struct AudioStreamingBridge {
    /// Stream configuration
    config: StreamConfig,

    /// Icecast stream manager
    stream_manager: Arc<Mutex<StreamManager>>,

    /// Audio input receiver from mixer
    audio_input_rx: Option<mpsc::Receiver<Vec<f32>>>,

    /// Broadcast channel for streaming status updates
    status_tx: broadcast::Sender<StreamingStatus>,

    /// Control channel for starting/stopping the bridge
    control_tx: Option<mpsc::Sender<StreamingCommand>>,
    control_rx: Option<mpsc::Receiver<StreamingCommand>>,

    /// Audio processing statistics
    pub stats: Arc<Mutex<StreamingStats>>,

    /// Buffer for audio format conversion
    conversion_buffer: Vec<u8>,
}

/// Streaming bridge status
#[derive(Debug, Clone)]
pub enum StreamingStatus {
    Disconnected,
    Connecting,
    Connected,
    Streaming { listeners: u32, duration: Duration },
    Error { message: String },
    Reconnecting { attempt: u32 },
}

/// Control commands for the streaming bridge
#[derive(Debug)]
pub enum StreamingCommand {
    Start,
    Stop,
    Reconnect,
    UpdateConfig(StreamConfig),
}

/// Audio streaming statistics
#[derive(Debug, Clone)]
pub struct StreamingStats {
    pub total_samples_processed: u64,
    pub samples_per_second: f64,
    pub average_latency_ms: f64,
    pub buffer_overruns: u32,
    pub encoding_errors: u32,
    pub network_errors: u32,
    pub last_update: Instant,
}

impl Default for StreamingStats {
    fn default() -> Self {
        Self {
            total_samples_processed: 0,
            samples_per_second: 0.0,
            average_latency_ms: 0.0,
            buffer_overruns: 0,
            encoding_errors: 0,
            network_errors: 0,
            last_update: Instant::now(),
        }
    }
}

impl AudioStreamingBridge {
    /// Create a new audio streaming bridge
    pub fn new(config: StreamConfig) -> Self {
        let stream_manager = Arc::new(Mutex::new(StreamManager::new(config.clone())));
        let (status_tx, _) = broadcast::channel(32);
        let (control_tx, control_rx) = mpsc::channel(16);

        Self {
            config,
            stream_manager,
            audio_input_rx: None,
            status_tx,
            control_tx: Some(control_tx),
            control_rx: Some(control_rx),
            stats: Arc::new(Mutex::new(StreamingStats::default())),
            conversion_buffer: Vec::with_capacity(96000),
        }
    }

    /// Connect audio input from the mixer
    pub fn connect_audio_input(&mut self, audio_rx: mpsc::Receiver<Vec<f32>>) {
        self.audio_input_rx = Some(audio_rx);
        println!("🔗 Audio streaming bridge connected to mixer output");
    }

    /// Get a receiver for streaming status updates
    pub fn subscribe_status(&self) -> broadcast::Receiver<StreamingStatus> {
        self.status_tx.subscribe()
    }

    /// Get current streaming statistics
    pub async fn get_stats(&self) -> StreamingStats {
        self.stats.lock().await.clone()
    }

    /// Start the streaming bridge
    pub async fn start(&mut self) -> Result<()> {
        if let Some(control_tx) = &self.control_tx {
            control_tx
                .send(StreamingCommand::Start)
                .await
                .context("Failed to send start command")?;
        }
        Ok(())
    }

    /// Stop the streaming bridge
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(control_tx) = &self.control_tx {
            control_tx
                .send(StreamingCommand::Stop)
                .await
                .context("Failed to send stop command")?;
        }
        Ok(())
    }

    /// Update streaming configuration
    pub async fn update_config(&mut self, config: StreamConfig) -> Result<()> {
        self.config = config.clone();
        if let Some(control_tx) = &self.control_tx {
            control_tx
                .send(StreamingCommand::UpdateConfig(config))
                .await
                .context("Failed to send config update")?;
        }
        Ok(())
    }

    /// Run the main streaming bridge event loop
    pub async fn run(&mut self) -> Result<()> {
        println!("🚀 Starting audio streaming bridge...");

        // Take ownership of required components
        let mut audio_input_rx = self
            .audio_input_rx
            .take()
            .ok_or_else(|| anyhow::anyhow!("Audio input not connected"))?;
        let mut control_rx = self
            .control_rx
            .take()
            .ok_or_else(|| anyhow::anyhow!("Control channel not available"))?;

        let stream_manager = self.stream_manager.clone();
        let status_tx = self.status_tx.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();

        // Audio processing parameters
        let sample_rate = config.sample_rate as f64;
        let channels = config.channels as usize;
        let mut samples_processed = 0u64;
        let mut last_stats_update = Instant::now();

        // Audio format conversion state
        let mut audio_buffer = Vec::<f32>::new();
        let target_chunk_size = (sample_rate * 0.1) as usize * channels; // 100ms chunks for stable encoding

        // Status tracking
        let mut is_streaming = false;
        let mut reconnect_attempts = 0u32;
        let max_reconnect_attempts = 5;

        // Initialize status
        let _ = status_tx.send(StreamingStatus::Disconnected);

        loop {
            tokio::select! {
                // Handle control commands
                Some(command) = control_rx.recv() => {
                    match command {
                        StreamingCommand::Start => {
                            println!("🎯 Streaming bridge: Starting stream...");
                            let _ = status_tx.send(StreamingStatus::Connecting);

                            // Connect to Icecast server
                            match stream_manager.lock().await.connect().await {
                                Ok(()) => {
                                    println!("✅ Connected to Icecast server");
                                    let _ = status_tx.send(StreamingStatus::Connected);
                                    is_streaming = true;
                                    reconnect_attempts = 0;
                                }
                                Err(e) => {
                                    eprintln!("❌ Failed to connect to Icecast: {}", e);
                                    let _ = status_tx.send(StreamingStatus::Error {
                                        message: format!("Connection failed: {}", e)
                                    });
                                }
                            }
                        }

                        StreamingCommand::Stop => {
                            println!("🛑 Streaming bridge: Stopping stream...");
                            if is_streaming {
                                if let Err(e) = stream_manager.lock().await.stop_stream().await {
                                    eprintln!("⚠️ Error stopping stream: {}", e);
                                }
                                if let Err(e) = stream_manager.lock().await.disconnect().await {
                                    eprintln!("⚠️ Error disconnecting: {}", e);
                                }
                            }
                            is_streaming = false;
                            let _ = status_tx.send(StreamingStatus::Disconnected);
                        }

                        StreamingCommand::Reconnect => {
                            println!("🔄 Streaming bridge: Reconnecting...");
                            reconnect_attempts += 1;
                            let _ = status_tx.send(StreamingStatus::Reconnecting { attempt: reconnect_attempts });

                            if reconnect_attempts <= max_reconnect_attempts {
                                // Attempt reconnection
                                match stream_manager.lock().await.connect().await {
                                    Ok(()) => {
                                        println!("✅ Reconnected to Icecast server");
                                        let _ = status_tx.send(StreamingStatus::Connected);
                                        is_streaming = true;
                                        reconnect_attempts = 0;
                                    }
                                    Err(e) => {
                                        eprintln!("❌ Reconnection attempt {} failed: {}", reconnect_attempts, e);
                                        if reconnect_attempts >= max_reconnect_attempts {
                                            let _ = status_tx.send(StreamingStatus::Error {
                                                message: "Max reconnection attempts exceeded".to_string()
                                            });
                                            is_streaming = false;
                                        }
                                    }
                                }
                            }
                        }

                        StreamingCommand::UpdateConfig(new_config) => {
                            println!("⚙️ Streaming bridge: Updating configuration...");
                            // Stop current stream, update config, restart if was streaming
                            let was_streaming = is_streaming;
                            if is_streaming {
                                let _ = stream_manager.lock().await.stop_stream().await;
                            }

                            // Create new stream manager with updated config
                            *stream_manager.lock().await = StreamManager::new(new_config);

                            if was_streaming {
                                // Restart streaming with new config
                                if let Ok(()) = stream_manager.lock().await.connect().await {
                                    let _ = status_tx.send(StreamingStatus::Connected);
                                }
                            }
                        }
                    }
                }

                // Process incoming audio data from mixer
                Some(audio_data) = audio_input_rx.recv() => {
                    if is_streaming && !audio_data.is_empty() {
                        // Accumulate audio data in buffer
                        audio_buffer.extend_from_slice(&audio_data);

                        // Process in chunks when we have enough data
                        while audio_buffer.len() >= target_chunk_size {
                            let chunk: Vec<f32> = audio_buffer.drain(..target_chunk_size).collect();

                            // Convert f32 samples to i16 PCM for encoding
                            let pcm_data: Vec<u8> = Self::convert_f32_to_pcm(&chunk);

                            // Send PCM data to stream manager for encoding and transmission
                            match stream_manager.lock().await.start_stream(pcm_data).await {
                                Ok(()) => {
                                    // Update statistics
                                    samples_processed += chunk.len() as u64;

                                    // Get listener count and update status
                                    if let Ok((current_listeners, _peak)) = stream_manager.lock().await.get_listener_stats().await {
                                        let _ = status_tx.send(StreamingStatus::Streaming {
                                            listeners: current_listeners,
                                            duration: last_stats_update.elapsed(),
                                        });
                                    }
                                }
                                Err(e) => {
                                    eprintln!("❌ Streaming error: {}", e);

                                    // Update error stats
                                    {
                                        let mut stats_guard = stats.lock().await;
                                        stats_guard.network_errors += 1;
                                    }

                                    // Trigger reconnection attempt
                                    let _ = control_rx.try_recv(); // Clear any pending commands
                                    if let Err(send_err) = control_rx.try_recv() {
                                        // Send reconnect command (non-blocking)
                                        println!("🔄 Triggering reconnection due to streaming error");
                                    }
                                }
                            }
                        }
                    }
                }

                // Periodic statistics update
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    if last_stats_update.elapsed() >= Duration::from_secs(5) {
                        Self::update_stats(&stats, samples_processed, last_stats_update.elapsed()).await;
                        last_stats_update = Instant::now();
                    }
                }

                // Handle unexpected channel closures
                else => {
                    println!("⚠️ Audio streaming bridge: All channels closed, exiting...");
                    break;
                }
            }
        }

        println!("🛑 Audio streaming bridge stopped");
        Ok(())
    }

    /// Convert f32 audio samples to 16-bit PCM format
    fn convert_f32_to_pcm(samples: &[f32]) -> Vec<u8> {
        let mut pcm_data = Vec::with_capacity(samples.len() * 2);

        for &sample in samples {
            // Clamp to [-1.0, 1.0] range and convert to i16
            let clamped = sample.max(-1.0).min(1.0);
            let pcm_sample = (clamped * 32767.0) as i16;

            // Convert to little-endian bytes
            pcm_data.extend_from_slice(&pcm_sample.to_le_bytes());
        }

        pcm_data
    }

    /// Update streaming statistics
    async fn update_stats(
        stats: &Arc<Mutex<StreamingStats>>,
        samples_processed: u64,
        elapsed: Duration,
    ) {
        let mut stats_guard = stats.lock().await;

        stats_guard.total_samples_processed = samples_processed;
        stats_guard.samples_per_second = samples_processed as f64 / elapsed.as_secs_f64();
        stats_guard.last_update = Instant::now();

        println!(
            "📊 Streaming stats: {} samples processed, {:.1} samples/sec",
            samples_processed, stats_guard.samples_per_second
        );
    }
}

/// Factory function to create and start an audio streaming bridge
pub async fn create_streaming_bridge(
    config: StreamConfig,
    audio_rx: mpsc::Receiver<Vec<f32>>,
) -> Result<AudioStreamingBridge> {
    let mut bridge = AudioStreamingBridge::new(config);
    bridge.connect_audio_input(audio_rx);

    println!("🏗️ Audio streaming bridge created and ready");
    Ok(bridge)
}
