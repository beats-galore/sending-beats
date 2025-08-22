use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Instant, Duration, sleep};
use tracing::{info, warn, error};

use crate::audio::{VirtualMixer, AudioStreamingBridge, create_streaming_bridge};
use crate::icecast_source::{IcecastStreamManager, AudioFormat, AudioCodec};
use crate::streaming::{StreamConfig, AudioEncoder};

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
    /// Audio mixer reference
    mixer: Arc<RwLock<Option<Arc<VirtualMixer>>>>,
    
    /// Icecast stream manager
    icecast_manager: Arc<Mutex<Option<IcecastStreamManager>>>,
    
    /// Audio streaming bridge
    streaming_bridge: Arc<Mutex<Option<AudioStreamingBridge>>>,
    
    /// Direct reference to streaming stats for efficient access
    streaming_stats: Arc<Mutex<Option<Arc<Mutex<crate::audio::streaming_bridge::StreamingStats>>>>>,
    
    /// Service state
    state: Arc<Mutex<ServiceState>>,
    
    /// Configuration
    config: Arc<RwLock<Option<StreamingServiceConfig>>>,
    
    /// Connection monitor task handle
    monitor_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

#[derive(Debug, Clone)]
pub struct StreamingServiceConfig {
    /// Icecast server configuration
    pub server_host: String,
    pub server_port: u16,
    pub mount_point: String,
    pub password: String,
    
    /// Stream metadata
    pub stream_name: String,
    pub stream_description: String,
    pub stream_genre: String,
    pub stream_url: String,
    pub is_public: bool,
    
    /// Audio encoding settings
    pub audio_format: AudioFormat,
    
    /// Available bitrate presets
    pub available_bitrates: Vec<u32>,
    pub selected_bitrate: u32,
    
    /// Variable bitrate settings
    pub enable_variable_bitrate: bool,
    pub vbr_quality: u8, // VBR quality level 0-9 (MP3 standard)
    
    /// Advanced settings
    pub auto_reconnect: bool,
    pub max_reconnect_attempts: u32,
    pub reconnect_delay_ms: u64,
}

impl Default for StreamingServiceConfig {
    fn default() -> Self {
        Self {
            server_host: "localhost".to_string(),
            server_port: 8000,
            mount_point: "/live".to_string(),
            password: std::env::var("ICECAST_PASSWORD").unwrap_or_else(|_| "changeme".to_string()),
            stream_name: "Sendin Beats Radio".to_string(),
            stream_description: "Live radio stream from Sendin Beats".to_string(),
            stream_genre: "Electronic".to_string(),
            stream_url: "https://sendinbeats.com".to_string(),
            is_public: true,
            audio_format: AudioFormat {
                sample_rate: 48000,
                channels: 2,
                bitrate: 192,
                codec: AudioCodec::Mp3,
            },
            available_bitrates: vec![96, 128, 160, 192, 256, 320],
            selected_bitrate: 192,
            enable_variable_bitrate: false,
            vbr_quality: 2, // High quality (V2) by default
            auto_reconnect: true,
            max_reconnect_attempts: 5,
            reconnect_delay_ms: 3000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionHealth {
    pub latency_ms: Option<u64>,
    pub packet_loss_rate: f32,
    pub last_heartbeat: Option<Instant>,
    pub consecutive_failures: u32,
    pub average_bitrate_kbps: f64,
    pub buffer_underruns: u32,
}

impl Default for ConnectionHealth {
    fn default() -> Self {
        Self {
            latency_ms: None,
            packet_loss_rate: 0.0,
            last_heartbeat: None,
            consecutive_failures: 0,
            average_bitrate_kbps: 0.0,
            buffer_underruns: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServiceState {
    pub is_running: bool,
    pub is_connected: bool,
    pub is_streaming: bool,
    pub last_error: Option<String>,
    pub start_time: Option<Instant>,
    pub reconnect_attempts: u32,
    pub last_connection_time: Option<Instant>,
    pub last_disconnect_time: Option<Instant>,
    pub connection_health: ConnectionHealth,
    pub should_auto_reconnect: bool,
}

impl Default for ServiceState {
    fn default() -> Self {
        Self {
            is_running: false,
            is_connected: false,
            is_streaming: false,
            last_error: None,
            start_time: None,
            reconnect_attempts: 0,
            last_connection_time: None,
            last_disconnect_time: None,
            connection_health: ConnectionHealth::default(),
            should_auto_reconnect: true,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StreamingServiceStatus {
    pub is_running: bool,
    pub is_connected: bool,
    pub is_streaming: bool,
    pub uptime_seconds: u64,
    pub audio_stats: Option<AudioStreamingStats>,
    pub icecast_stats: Option<IcecastStreamingStats>,
    pub connection_diagnostics: ConnectionDiagnostics,
    pub bitrate_info: BitrateInfo,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BitrateInfo {
    pub current_bitrate: u32,
    pub available_bitrates: Vec<u32>,
    pub codec: String,
    pub is_variable_bitrate: bool,
    pub vbr_quality: u8,
    pub actual_bitrate: Option<u32>, // Real-time bitrate for VBR
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionDiagnostics {
    pub latency_ms: Option<u64>,
    pub packet_loss_rate: f32,
    pub connection_stability: f32, // 0.0 to 1.0
    pub reconnect_attempts: u32,
    pub time_since_last_reconnect_seconds: Option<u64>,
    pub connection_uptime_seconds: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioStreamingStats {
    pub samples_processed: u64,
    pub samples_per_second: f64,
    pub buffer_overruns: u32,
    pub encoding_errors: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IcecastStreamingStats {
    pub bytes_sent: u64,
    pub packets_sent: u64,
    pub connection_duration_seconds: u64,
    pub average_bitrate_kbps: f64,
}

impl StreamingService {
    /// Create a new streaming service
    pub fn new() -> Self {
        Self {
            mixer: Arc::new(RwLock::new(None)),
            icecast_manager: Arc::new(Mutex::new(None)),
            streaming_bridge: Arc::new(Mutex::new(None)),
            streaming_stats: Arc::new(Mutex::new(None)),
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
    
    /// Connect to the audio mixer
    pub async fn connect_mixer(&self, mixer: Arc<VirtualMixer>) -> Result<()> {
        info!("🔗 Connecting streaming service to audio mixer...");
        
        // Store mixer reference
        *self.mixer.write().await = Some(mixer.clone());
        
        // Get audio output from mixer
        let audio_rx = mixer.create_streaming_audio_receiver().await;
        
        // Create audio streaming bridge
        let config = self.config.read().await;
        if let Some(ref cfg) = *config {
            let stream_config = StreamConfig {
                icecast_url: format!("http://{}:{}", cfg.server_host, cfg.server_port),
                mount_point: cfg.mount_point.clone(),
                username: "source".to_string(),
                password: cfg.password.clone(),
                bitrate: cfg.audio_format.bitrate,
                sample_rate: cfg.audio_format.sample_rate,
                channels: cfg.audio_format.channels,
            };
            
            let bridge = create_streaming_bridge(stream_config, audio_rx).await?;
            
            // Store stats reference for efficient access
            *self.streaming_stats.lock().await = Some(bridge.stats.clone());
            
            // Connect audio input to Icecast manager
            if let Some(ref mut icecast_manager) = *self.icecast_manager.lock().await {
                // Create a channel to connect bridge to Icecast manager
                let (encoder_tx, encoder_rx) = tokio::sync::mpsc::channel(512);
                icecast_manager.connect_audio_input(encoder_rx);
                
                // Spawn encoding task to convert f32 audio to encoded format
                let audio_format = cfg.audio_format.clone();
                tokio::spawn(async move {
                    Self::run_audio_encoder(bridge, encoder_tx, audio_format).await;
                });
            }
            
            info!("✅ Streaming service connected to mixer");
        } else {
            return Err(anyhow::anyhow!("Streaming service not initialized"));
        }
        
        Ok(())
    }
    
    /// Connect to the audio mixer using a reference
    pub async fn connect_mixer_ref(&self, mixer: &VirtualMixer) -> Result<()> {
        info!("🔗 Connecting streaming service to audio mixer (ref)...");
        
        // Get audio output from mixer
        let audio_rx = mixer.create_streaming_audio_receiver().await;
        
        // Create audio streaming bridge
        let config = self.config.read().await;
        if let Some(ref cfg) = *config {
            let stream_config = StreamConfig {
                icecast_url: format!("http://{}:{}", cfg.server_host, cfg.server_port),
                mount_point: cfg.mount_point.clone(),
                username: "source".to_string(),
                password: cfg.password.clone(),
                bitrate: cfg.audio_format.bitrate,
                sample_rate: cfg.audio_format.sample_rate,
                channels: cfg.audio_format.channels,
            };
            
            let _bridge = create_streaming_bridge(stream_config, audio_rx).await?;
            
            // Connect audio input to Icecast manager
            if let Some(ref mut icecast_manager) = *self.icecast_manager.lock().await {
                // Create a channel to connect bridge to Icecast manager
                let (_encoder_tx, encoder_rx) = tokio::sync::mpsc::channel(512);
                icecast_manager.connect_audio_input(encoder_rx);
                
                // Note: In a full implementation, we'd spawn the audio encoder task here
                // For now, this creates the connection but doesn't start the encoding pipeline
            }
            
            info!("✅ Streaming service connected to mixer (ref)");
        } else {
            return Err(anyhow::anyhow!("Streaming service not initialized"));
        }
        
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
        let uptime = state.start_time
            .map(|start| start.elapsed().as_secs())
            .unwrap_or(0);
        
        // Get audio streaming stats from stored stats reference
        let audio_stats = if let Some(ref stats_ref) = *self.streaming_stats.lock().await {
            let bridge_stats = stats_ref.lock().await;
            Some(AudioStreamingStats {
                samples_processed: bridge_stats.total_samples_processed,
                samples_per_second: bridge_stats.samples_per_second,
                buffer_overruns: bridge_stats.buffer_overruns,
                encoding_errors: bridge_stats.encoding_errors,
            })
        } else {
            None
        };

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
            time_since_last_reconnect_seconds: state.last_connection_time
                .map(|time| time.elapsed().as_secs()),
            connection_uptime_seconds: state.last_connection_time
                .map(|time| time.elapsed().as_secs()),
        };
        
        // Get bitrate information
        let bitrate_info = {
            let config = self.config.read().await;
            if let Some(ref cfg) = *config {
                // Get actual bitrate from Icecast stats if VBR is enabled
                let actual_bitrate = if cfg.enable_variable_bitrate {
                    icecast_stats.as_ref().map(|s| s.average_bitrate_kbps as u32)
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
                return Err(anyhow::anyhow!("Unsupported bitrate: {}kbps. Available: {:?}", 
                    bitrate, cfg.available_bitrates));
            }
            
            cfg.selected_bitrate = bitrate;
            cfg.audio_format.bitrate = bitrate;
            
            info!("✅ Bitrate set to {}kbps (restart streaming to apply)", bitrate);
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
        info!("🎵 Setting variable bitrate: enabled={}, quality=V{}", enabled, quality);
        
        let mut config = self.config.write().await;
        if let Some(ref mut cfg) = *config {
            // Validate quality range (0-9 for MP3 VBR)
            let clamped_quality = quality.clamp(0, 9);
            if clamped_quality != quality {
                warn!("VBR quality clamped from {} to {}", quality, clamped_quality);
            }
            
            cfg.enable_variable_bitrate = enabled;
            cfg.vbr_quality = clamped_quality;
            
            info!("✅ Variable bitrate set: enabled={}, quality=V{} (restart streaming to apply)", 
                enabled, clamped_quality);
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
    pub fn create_bitrate_preset(bitrate: u32, codec: AudioCodec) -> Result<StreamingServiceConfig> {
        let mut config = StreamingServiceConfig::default();
        
        if !config.available_bitrates.contains(&bitrate) {
            return Err(anyhow::anyhow!("Unsupported bitrate: {}kbps", bitrate));
        }
        
        config.selected_bitrate = bitrate;
        config.audio_format.bitrate = bitrate;
        config.audio_format.codec = codec;
        
        // Adjust sample rate based on bitrate for optimal quality
        config.audio_format.sample_rate = match bitrate {
            96 | 128 => 44100,  // Lower bitrates work fine with 44.1kHz
            _ => 48000,         // Higher bitrates benefit from 48kHz
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
                Self::check_connection_health(
                    &state_ref,
                    &icecast_manager_ref,
                    &config,
                ).await;
                
                // Handle auto-reconnect if needed
                Self::handle_auto_reconnect(
                    &state_ref,
                    &icecast_manager_ref,
                    &config,
                ).await;
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
    
    /// Audio encoder task - converts f32 audio to encoded format
    async fn run_audio_encoder(
        bridge: AudioStreamingBridge,
        encoder_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
        audio_format: AudioFormat,
    ) {
        info!("🎵 Starting audio encoder task...");
        
        // Create audio encoder
        let encoder = AudioEncoder::new(
            audio_format.bitrate,
            audio_format.sample_rate,
            audio_format.channels,
        );
        
        // Get audio from streaming bridge
        let audio_rx = bridge.subscribe_status(); // This should be audio data receiver
        
        // Note: This is a simplified implementation
        // In practice, we'd need to properly integrate the AudioStreamingBridge
        // with the encoding pipeline
        
        info!("🎵 Audio encoder task stopped");
    }
}

/// Global streaming service instance
static STREAMING_SERVICE: tokio::sync::OnceCell<Arc<StreamingService>> = tokio::sync::OnceCell::const_new();

/// Get or create the global streaming service
pub async fn get_streaming_service() -> Arc<StreamingService> {
    STREAMING_SERVICE.get_or_init(|| async {
        Arc::new(StreamingService::new())
    }).await.clone()
}

/// Initialize streaming with configuration
pub async fn initialize_streaming(config: StreamingServiceConfig) -> Result<()> {
    let service = get_streaming_service().await;
    service.initialize(config).await
}

/// Connect streaming to mixer
pub async fn connect_streaming_to_mixer(mixer: &VirtualMixer) -> Result<()> {
    let service = get_streaming_service().await;
    service.connect_mixer_ref(mixer).await
}

/// Start streaming
pub async fn start_streaming() -> Result<()> {
    let service = get_streaming_service().await;
    service.start_streaming().await
}

/// Stop streaming
pub async fn stop_streaming() -> Result<()> {
    let service = get_streaming_service().await;
    service.stop_streaming().await
}

/// Update stream metadata
pub async fn update_stream_metadata(title: String, artist: String) -> Result<()> {
    let service = get_streaming_service().await;
    service.update_metadata(title, artist).await
}

/// Get streaming status
pub async fn get_streaming_status() -> StreamingServiceStatus {
    let service = get_streaming_service().await;
    service.get_status().await
}

/// Set stream bitrate
pub async fn set_stream_bitrate(bitrate: u32) -> Result<()> {
    let service = get_streaming_service().await;
    service.set_bitrate(bitrate).await
}

/// Get available bitrates
pub async fn get_available_bitrates() -> Vec<u32> {
    let service = get_streaming_service().await;
    service.get_available_bitrates().await
}

/// Get current bitrate
pub async fn get_current_stream_bitrate() -> u32 {
    let service = get_streaming_service().await;
    service.get_current_bitrate().await
}

/// Create bitrate preset configuration
pub fn create_stream_bitrate_preset(bitrate: u32, codec: AudioCodec) -> Result<StreamingServiceConfig> {
    StreamingService::create_bitrate_preset(bitrate, codec)
}

/// Set variable bitrate streaming
pub async fn set_variable_bitrate_streaming(enabled: bool, quality: u8) -> Result<()> {
    let service = get_streaming_service().await;
    service.set_variable_bitrate(enabled, quality).await
}

/// Get variable bitrate settings
pub async fn get_variable_bitrate_settings() -> (bool, u8) {
    let service = get_streaming_service().await;
    service.get_variable_bitrate_settings().await
}