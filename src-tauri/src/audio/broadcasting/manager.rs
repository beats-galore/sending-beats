use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, Duration, Instant};
use tracing::{error, info, warn};

use super::config::StreamingServiceConfig;
use super::icecast_source::{AudioCodec, IcecastStreamManager};
use super::types::{
    AudioStreamingStats, BitrateInfo, ConnectionDiagnostics, ConnectionHealth,
    IcecastStreamingStats, ServiceState, StreamingServiceStatus,
};

/// Integrated streaming service that connects the mixer to Icecast
///
/// This service manages the complete audio streaming pipeline:
/// 1. Captures real-time audio from the virtual mixer
/// 2. Encodes audio to MP3/AAC format
/// 3. Streams to Icecast server using SOURCE protocol
/// 4. Handles reconnection and error recovery
/// 5. Provides streaming statistics and status
#[derive(Debug)]
pub struct StreamingService {
    /// Icecast stream manager
    icecast_manager: Arc<Mutex<Option<IcecastStreamManager>>>,

    /// Service state
    state: Arc<Mutex<ServiceState>>,

    /// Configuration
    config: Arc<RwLock<Option<StreamingServiceConfig>>>,

    /// Connection monitor task handle
    monitor_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl StreamingService {
    /// Create a new streaming service
    pub fn new() -> Self {
        Self {
            icecast_manager: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(ServiceState::default())),
            config: Arc::new(RwLock::new(None)),
            monitor_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize the streaming service with configuration
    pub async fn initialize(&self, config: StreamingServiceConfig) -> Result<()> {
        info!("🔧 Initializing streaming service...");

        // Store configuration
        *self.config.write().await = Some(config.clone());

        // Create Icecast stream manager
        let icecast_manager = IcecastStreamManager::new(
            config.server_host.clone(),
            config.server_port,
            config.mount_point.clone(),
            config.password.clone(),
            config.audio_format.clone(),
        );

        *self.icecast_manager.lock().await = Some(icecast_manager);

        info!("✅ Streaming service initialized");
        Ok(())
    }

    /// Start streaming
    pub async fn start_streaming(&self) -> Result<()> {
        info!("🎯 Starting streaming...");

        // Update state
        {
            let mut state = self.state.lock().await;
            state.is_running = true;
            state.start_time = Some(Instant::now());
            state.reconnect_attempts = 0;
            state.last_error = None;
            state.should_auto_reconnect = true;
        }

        // Start Icecast manager
        if let Some(ref mut icecast_manager) = *self.icecast_manager.lock().await {
            icecast_manager.start_streaming().await?;

            // Update connection state
            {
                let mut state = self.state.lock().await;
                state.is_connected = true;
                state.is_streaming = true;
                state.last_connection_time = Some(Instant::now());
                state.connection_health.last_heartbeat = Some(Instant::now());
                state.connection_health.consecutive_failures = 0;
            }

            // Start connection monitor
            self.start_connection_monitor().await;
        } else {
            return Err(anyhow::anyhow!("Icecast manager not initialized"));
        }

        info!("✅ Streaming started successfully");
        Ok(())
    }

    /// Stop streaming
    pub async fn stop_streaming(&self) -> Result<()> {
        info!("🛑 Stopping streaming...");

        // Stop connection monitor
        if let Some(handle) = self.monitor_handle.lock().await.take() {
            handle.abort();
        }

        // Stop Icecast manager
        if let Some(ref mut icecast_manager) = *self.icecast_manager.lock().await {
            icecast_manager.stop_streaming().await?;
        }

        // Update state
        {
            let mut state = self.state.lock().await;
            state.is_running = false;
            state.is_connected = false;
            state.is_streaming = false;
            state.should_auto_reconnect = false;
            state.last_disconnect_time = Some(Instant::now());
        }

        info!("✅ Streaming stopped");
        Ok(())
    }

    /// Start streaming with an RTRB consumer from the audio pipeline
    pub async fn start_streaming_with_consumer(
        &self,
        config: StreamingServiceConfig,
        rtrb_consumer: rtrb::Consumer<f32>,
    ) -> Result<()> {
        info!("🎯 Starting streaming with RTRB consumer from audio pipeline...");

        // Initialize with the provided config first
        self.initialize(config).await?;

        // Update state
        {
            let mut state = self.state.lock().await;
            state.is_running = true;
            state.start_time = Some(Instant::now());
            state.reconnect_attempts = 0;
            state.last_error = None;
            state.should_auto_reconnect = true;
        }

        // Start Icecast manager with the RTRB consumer
        if let Some(ref mut icecast_manager) = *self.icecast_manager.lock().await {
            icecast_manager
                .start_streaming_with_consumer(rtrb_consumer)
                .await?;

            // Update connection state
            {
                let mut state = self.state.lock().await;
                state.is_connected = true;
                state.is_streaming = true;
                state.last_connection_time = Some(Instant::now());
                state.connection_health.last_heartbeat = Some(Instant::now());
                state.connection_health.consecutive_failures = 0;
            }

            // Start connection monitor
            self.start_connection_monitor().await;
        } else {
            return Err(anyhow::anyhow!("Icecast manager not initialized"));
        }

        info!("✅ Streaming started successfully with RTRB consumer");
        Ok(())
    }

    /// Update stream metadata
    pub async fn update_metadata(&self, title: String, artist: String) -> Result<()> {
        info!("📝 Updating stream metadata: {} - {}", artist, title);

        if let Some(ref mut icecast_manager) = *self.icecast_manager.lock().await {
            icecast_manager.update_metadata(title, artist).await?;
        }

        Ok(())
    }

    /// Get streaming service status
    pub async fn get_status(&self) -> StreamingServiceStatus {
        let state = self.state.lock().await;
        let uptime = state
            .start_time
            .map(|start| start.elapsed().as_secs())
            .unwrap_or(0);

        // The bridge that fed these stats never ran; the field is kept in the
        // status shape for the frontend but is always empty.
        let audio_stats: Option<AudioStreamingStats> = None;

        // Get Icecast stats
        let icecast_stats = if let Some(ref icecast_manager) = *self.icecast_manager.lock().await {
            let stats = icecast_manager.get_stats();
            Some(IcecastStreamingStats {
                bytes_sent: stats.bytes_sent,
                packets_sent: stats.packets_sent,
                connection_duration_seconds: stats.connection_duration.as_secs(),
                average_bitrate_kbps: stats.average_bitrate_kbps,
            })
        } else {
            None
        };

        // Calculate connection diagnostics
        let connection_diagnostics = ConnectionDiagnostics {
            latency_ms: state.connection_health.latency_ms,
            packet_loss_rate: state.connection_health.packet_loss_rate,
            connection_stability: Self::calculate_connection_stability(&state.connection_health),
            reconnect_attempts: state.reconnect_attempts,
            time_since_last_reconnect_seconds: state
                .last_connection_time
                .map(|time| time.elapsed().as_secs()),
            connection_uptime_seconds: state
                .last_connection_time
                .map(|time| time.elapsed().as_secs()),
        };

        // Get bitrate information
        let bitrate_info = {
            let config = self.config.read().await;
            if let Some(ref cfg) = *config {
                // Get actual bitrate from Icecast stats if VBR is enabled
                let actual_bitrate = if cfg.enable_variable_bitrate {
                    icecast_stats
                        .as_ref()
                        .map(|s| s.average_bitrate_kbps as u32)
                } else {
                    None
                };

                BitrateInfo {
                    current_bitrate: cfg.selected_bitrate,
                    available_bitrates: cfg.available_bitrates.clone(),
                    codec: match cfg.audio_format.codec {
                        AudioCodec::Mp3 => "MP3".to_string(),
                        AudioCodec::Aac => "AAC".to_string(),
                        AudioCodec::Ogg => "OGG".to_string(),
                    },
                    is_variable_bitrate: cfg.enable_variable_bitrate,
                    vbr_quality: cfg.vbr_quality,
                    actual_bitrate,
                }
            } else {
                BitrateInfo {
                    current_bitrate: 192,
                    available_bitrates: vec![96, 128, 160, 192, 256, 320],
                    codec: "MP3".to_string(),
                    is_variable_bitrate: false,
                    vbr_quality: 2,
                    actual_bitrate: None,
                }
            }
        };

        StreamingServiceStatus {
            is_running: state.is_running,
            is_connected: state.is_connected,
            is_streaming: state.is_streaming,
            uptime_seconds: uptime,
            audio_stats,
            icecast_stats,
            connection_diagnostics,
            bitrate_info,
            last_error: state.last_error.clone(),
        }
    }

    /// Set stream bitrate (requires restart to take effect)
    pub async fn set_bitrate(&self, bitrate: u32) -> Result<()> {
        info!("🎵 Setting stream bitrate to {}kbps", bitrate);

        let mut config = self.config.write().await;
        if let Some(ref mut cfg) = *config {
            // Check if bitrate is supported
            if !cfg.available_bitrates.contains(&bitrate) {
                return Err(anyhow::anyhow!(
                    "Unsupported bitrate: {}kbps. Available: {:?}",
                    bitrate,
                    cfg.available_bitrates
                ));
            }

            cfg.selected_bitrate = bitrate;
            cfg.audio_format.bitrate = bitrate;

            info!(
                "✅ Bitrate set to {}kbps (restart streaming to apply)",
                bitrate
            );
        } else {
            return Err(anyhow::anyhow!("Streaming service not initialized"));
        }

        Ok(())
    }

    /// Get available bitrates
    pub async fn get_available_bitrates(&self) -> Vec<u32> {
        let config = self.config.read().await;
        if let Some(ref cfg) = *config {
            cfg.available_bitrates.clone()
        } else {
            vec![96, 128, 160, 192, 256, 320] // Default bitrates
        }
    }

    /// Get current selected bitrate
    pub async fn get_current_bitrate(&self) -> u32 {
        let config = self.config.read().await;
        if let Some(ref cfg) = *config {
            cfg.selected_bitrate
        } else {
            192 // Default bitrate
        }
    }

    /// Enable/disable variable bitrate streaming
    pub async fn set_variable_bitrate(&self, enabled: bool, quality: u8) -> Result<()> {
        info!(
            "🎵 Setting variable bitrate: enabled={}, quality=V{}",
            enabled, quality
        );

        let mut config = self.config.write().await;
        if let Some(ref mut cfg) = *config {
            // Validate quality range (0-9 for MP3 VBR)
            let clamped_quality = quality.clamp(0, 9);
            if clamped_quality != quality {
                warn!(
                    "VBR quality clamped from {} to {}",
                    quality, clamped_quality
                );
            }

            cfg.enable_variable_bitrate = enabled;
            cfg.vbr_quality = clamped_quality;

            info!(
                "✅ Variable bitrate set: enabled={}, quality=V{} (restart streaming to apply)",
                enabled, clamped_quality
            );
        } else {
            return Err(anyhow::anyhow!("Streaming service not initialized"));
        }

        Ok(())
    }

    /// Get variable bitrate settings
    pub async fn get_variable_bitrate_settings(&self) -> (bool, u8) {
        let config = self.config.read().await;
        if let Some(ref cfg) = *config {
            (cfg.enable_variable_bitrate, cfg.vbr_quality)
        } else {
            (false, 2) // Default settings (V2 - high quality)
        }
    }

    /// Create a preset configuration for a specific bitrate
    pub fn create_bitrate_preset(
        bitrate: u32,
        codec: AudioCodec,
    ) -> Result<StreamingServiceConfig> {
        let mut config = StreamingServiceConfig::default();

        if !config.available_bitrates.contains(&bitrate) {
            return Err(anyhow::anyhow!("Unsupported bitrate: {}kbps", bitrate));
        }

        config.selected_bitrate = bitrate;
        config.audio_format.bitrate = bitrate;
        config.audio_format.codec = codec;

        // Adjust sample rate based on bitrate for optimal quality
        config.audio_format.sample_rate = match bitrate {
            96 | 128 => 44100,                      // Lower bitrates work fine with 44.1kHz
            _ => crate::types::DEFAULT_SAMPLE_RATE, // Higher bitrates benefit from 48kHz
        };

        Ok(config)
    }

    /// Start connection monitoring task
    async fn start_connection_monitor(&self) {
        let state_ref = self.state.clone();
        let config_ref = self.config.clone();
        let icecast_manager_ref = self.icecast_manager.clone();

        let monitor_task = tokio::spawn(async move {
            info!("🔍 Starting connection monitor...");

            loop {
                sleep(Duration::from_secs(5)).await; // Check every 5 seconds

                let config = {
                    let config_guard = config_ref.read().await;
                    if let Some(ref cfg) = *config_guard {
                        cfg.clone()
                    } else {
                        continue;
                    }
                };

                // Check if we should continue monitoring
                let should_monitor = {
                    let state = state_ref.lock().await;
                    state.is_running && state.should_auto_reconnect
                };

                if !should_monitor {
                    info!("🔍 Connection monitor stopped");
                    break;
                }

                // Check connection health
                Self::check_connection_health(&state_ref, &icecast_manager_ref, &config).await;

                // Handle auto-reconnect if needed
                Self::handle_auto_reconnect(&state_ref, &icecast_manager_ref, &config).await;
            }
        });

        *self.monitor_handle.lock().await = Some(monitor_task);
    }

    /// Check connection health and update diagnostics
    async fn check_connection_health(
        state_ref: &Arc<Mutex<ServiceState>>,
        icecast_manager_ref: &Arc<Mutex<Option<IcecastStreamManager>>>,
        _config: &StreamingServiceConfig,
    ) {
        let mut state = state_ref.lock().await;

        // Update heartbeat
        state.connection_health.last_heartbeat = Some(Instant::now());

        // Check if connection is still alive by checking Icecast manager status
        if let Some(ref icecast_manager) = *icecast_manager_ref.lock().await {
            let stats = icecast_manager.get_stats();

            // Update bitrate from stats
            state.connection_health.average_bitrate_kbps = stats.average_bitrate_kbps;

            // Simple connection health check - if we're not getting data flow, mark as unhealthy
            if stats.bytes_sent == 0 && state.is_connected {
                state.connection_health.consecutive_failures += 1;
                warn!("🔍 Connection health check failed - no data flow detected");
            } else {
                state.connection_health.consecutive_failures = 0;
            }

            // If too many consecutive failures, mark as disconnected
            if state.connection_health.consecutive_failures >= 3 {
                warn!("🔍 Connection marked as failed due to consecutive failures");
                state.is_connected = false;
                state.is_streaming = false;
                state.last_disconnect_time = Some(Instant::now());
                state.last_error = Some("Connection health check failed".to_string());
            }
        }
    }

    /// Handle auto-reconnect logic
    async fn handle_auto_reconnect(
        state_ref: &Arc<Mutex<ServiceState>>,
        icecast_manager_ref: &Arc<Mutex<Option<IcecastStreamManager>>>,
        config: &StreamingServiceConfig,
    ) {
        let should_reconnect = {
            let state = state_ref.lock().await;
            !state.is_connected
                && state.is_running
                && config.auto_reconnect
                && state.reconnect_attempts < config.max_reconnect_attempts
        };

        if should_reconnect {
            info!("🔄 Attempting auto-reconnect...");

            // Wait before attempting reconnect
            sleep(Duration::from_millis(config.reconnect_delay_ms)).await;

            // Attempt reconnection
            if let Some(ref mut icecast_manager) = *icecast_manager_ref.lock().await {
                match icecast_manager.start_streaming().await {
                    Ok(()) => {
                        info!("✅ Auto-reconnect successful");
                        let mut state = state_ref.lock().await;
                        state.is_connected = true;
                        state.is_streaming = true;
                        state.last_connection_time = Some(Instant::now());
                        state.connection_health.last_heartbeat = Some(Instant::now());
                        state.connection_health.consecutive_failures = 0;
                        state.last_error = None;
                    }
                    Err(e) => {
                        error!("❌ Auto-reconnect failed: {}", e);
                        let mut state = state_ref.lock().await;
                        state.reconnect_attempts += 1;
                        state.last_error = Some(format!("Reconnect failed: {}", e));

                        if state.reconnect_attempts >= config.max_reconnect_attempts {
                            error!("❌ Max reconnect attempts reached, giving up");
                            state.should_auto_reconnect = false;
                        }
                    }
                }
            }
        }
    }

    /// Calculate connection stability score (0.0 to 1.0)
    fn calculate_connection_stability(health: &ConnectionHealth) -> f32 {
        // Base stability on consecutive failures and packet loss
        let failure_penalty = (health.consecutive_failures as f32 * 0.2).min(1.0);
        let packet_loss_penalty = health.packet_loss_rate;

        // Stability decreases with failures and packet loss
        (1.0 - failure_penalty - packet_loss_penalty).max(0.0)
    }
}
