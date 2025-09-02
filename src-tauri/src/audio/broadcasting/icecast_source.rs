use anyhow::{Context, Result};
use base64::Engine;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Icecast SOURCE protocol client implementation
///
/// This implements the proper Icecast SOURCE protocol for streaming audio data,
/// which is more efficient and reliable than HTTP POST requests.
#[derive(Debug)]
pub struct IcecastSourceClient {
    /// Server configuration
    server_host: String,
    server_port: u16,
    mount_point: String,
    password: String,

    /// Stream metadata
    stream_name: String,
    stream_description: String,
    stream_genre: String,
    stream_url: String,
    is_public: bool,

    /// Audio format
    audio_format: AudioFormat,

    /// Connection state
    connection: Option<TcpStream>,
    is_connected: bool,

    /// Statistics
    bytes_sent: u64,
    packets_sent: u64,
    connection_start: Option<std::time::Instant>,
}

/// Audio format configuration
#[derive(Debug, Clone)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub bitrate: u32,
    pub codec: AudioCodec,
}

#[derive(Debug, Clone)]
pub enum AudioCodec {
    Mp3,
    Aac,
    Ogg,
}

impl AudioCodec {
    fn mime_type(&self) -> &'static str {
        match self {
            AudioCodec::Mp3 => "audio/mpeg",
            AudioCodec::Aac => "audio/aac",
            AudioCodec::Ogg => "application/ogg",
        }
    }
}

impl IcecastSourceClient {
    /// Create a new Icecast SOURCE client
    pub fn new(
        server_host: String,
        server_port: u16,
        mount_point: String,
        password: String,
        audio_format: AudioFormat,
    ) -> Self {
        Self {
            server_host,
            server_port,
            mount_point,
            password,
            stream_name: "Sendin Beats Radio".to_string(),
            stream_description: "Live Radio Stream from Sendin Beats".to_string(),
            stream_genre: "Electronic".to_string(),
            stream_url: "https://sendinbeats.com".to_string(),
            is_public: true,
            audio_format,
            connection: None,
            is_connected: false,
            bytes_sent: 0,
            packets_sent: 0,
            connection_start: None,
        }
    }

    /// Set stream metadata
    pub fn set_metadata(
        &mut self,
        name: String,
        description: String,
        genre: String,
        url: String,
        is_public: bool,
    ) {
        self.stream_name = name;
        self.stream_description = description;
        self.stream_genre = genre;
        self.stream_url = url;
        self.is_public = is_public;
    }

    /// Connect to the Icecast server using SOURCE protocol
    pub async fn connect(&mut self) -> Result<()> {
        info!(
            "🔗 Connecting to Icecast server {}:{}",
            self.server_host, self.server_port
        );

        // Establish TCP connection
        let mut stream = TcpStream::connect(format!("{}:{}", self.server_host, self.server_port))
            .await
            .context("Failed to connect to Icecast server")?;

        // Send SOURCE request
        let source_request = self.build_source_request();
        stream
            .write_all(source_request.as_bytes())
            .await
            .context("Failed to send SOURCE request")?;

        // Read response
        let mut response_buffer = [0u8; 1024];
        let bytes_read = stream
            .read(&mut response_buffer)
            .await
            .context("Failed to read server response")?;

        let response = String::from_utf8_lossy(&response_buffer[..bytes_read]);
        debug!("Server response: {}", response);

        // Check if connection was accepted
        if response.starts_with("HTTP/1.1 200 OK") || response.starts_with("HTTP/1.0 200 OK") {
            info!("✅ Successfully connected to Icecast server");
            self.connection = Some(stream);
            self.is_connected = true;
            self.connection_start = Some(std::time::Instant::now());
            self.bytes_sent = 0;
            self.packets_sent = 0;
            Ok(())
        } else if response.contains("401") {
            error!("❌ Authentication failed - check password");
            Err(anyhow::anyhow!("Authentication failed: Invalid password"))
        } else if response.contains("403") {
            error!("❌ Mount point forbidden - check permissions");
            Err(anyhow::anyhow!(
                "Mount point forbidden: {}",
                self.mount_point
            ))
        } else {
            error!("❌ Icecast server rejected connection: {}", response.trim());
            Err(anyhow::anyhow!("Connection rejected: {}", response.trim()))
        }
    }

    /// Send audio data to the Icecast server
    pub async fn send_audio_data(&mut self, audio_data: &[u8]) -> Result<()> {
        if !self.is_connected {
            return Err(anyhow::anyhow!("Not connected to Icecast server"));
        }

        if let Some(ref mut connection) = self.connection {
            connection
                .write_all(audio_data)
                .await
                .context("Failed to send audio data")?;

            // Update statistics
            self.bytes_sent += audio_data.len() as u64;
            self.packets_sent += 1;

            // Log statistics periodically
            if self.packets_sent % 100 == 0 {
                let duration = self.connection_start.unwrap().elapsed();
                let bitrate = (self.bytes_sent * 8) as f64 / duration.as_secs_f64() / 1000.0;
                debug!(
                    "📊 Streaming stats: {} packets, {} bytes, {:.1} kbps",
                    self.packets_sent, self.bytes_sent, bitrate
                );
            }

            Ok(())
        } else {
            Err(anyhow::anyhow!("Connection lost"))
        }
    }

    /// Disconnect from the Icecast server
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(mut connection) = self.connection.take() {
            let _ = connection.shutdown().await;
            info!("🔌 Disconnected from Icecast server");
        }

        self.is_connected = false;
        self.connection_start = None;
        Ok(())
    }

    /// Check if client is connected
    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    /// Get streaming statistics
    pub fn get_stats(&self) -> IcecastStats {
        let duration = self
            .connection_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);

        let avg_bitrate = if duration.as_secs() > 0 {
            (self.bytes_sent * 8) as f64 / duration.as_secs_f64() / 1000.0
        } else {
            0.0
        };

        IcecastStats {
            bytes_sent: self.bytes_sent,
            packets_sent: self.packets_sent,
            connection_duration: duration,
            average_bitrate_kbps: avg_bitrate,
        }
    }

    /// Build the SOURCE protocol request
    fn build_source_request(&self) -> String {
        // Encode password in base64
        let auth_string = format!("source:{}", self.password);
        let auth_b64 = base64::engine::general_purpose::STANDARD.encode(auth_string.as_bytes());

        // Build SOURCE request with all headers
        format!(
            "SOURCE {} HTTP/1.0\r\n\
             Authorization: Basic {}\r\n\
             User-Agent: Sendin-Beats/1.0\r\n\
             Content-Type: {}\r\n\
             Ice-Name: {}\r\n\
             Ice-Description: {}\r\n\
             Ice-Genre: {}\r\n\
             Ice-URL: {}\r\n\
             Ice-Public: {}\r\n\
             Ice-Bitrate: {}\r\n\
             Ice-Channels: {}\r\n\
             Ice-Samplerate: {}\r\n\
             \r\n",
            self.mount_point,
            auth_b64,
            self.audio_format.codec.mime_type(),
            self.stream_name,
            self.stream_description,
            self.stream_genre,
            self.stream_url,
            if self.is_public { "1" } else { "0" },
            self.audio_format.bitrate,
            self.audio_format.channels,
            self.audio_format.sample_rate,
        )
    }
}

/// Icecast streaming statistics
#[derive(Debug, Clone)]
pub struct IcecastStats {
    pub bytes_sent: u64,
    pub packets_sent: u64,
    pub connection_duration: Duration,
    pub average_bitrate_kbps: f64,
}

/// Enhanced Icecast streaming manager with SOURCE protocol
#[derive(Debug)]
pub struct IcecastStreamManager {
    client: IcecastSourceClient,
    audio_rx: Option<mpsc::Receiver<Vec<u8>>>,
    control_tx: Option<mpsc::Sender<StreamControl>>,
    control_rx: Option<mpsc::Receiver<StreamControl>>,
    is_streaming: bool,
}

#[derive(Debug)]
pub enum StreamControl {
    Start,
    Stop,
    UpdateMetadata { title: String, artist: String },
}

impl IcecastStreamManager {
    /// Create a new Icecast stream manager
    pub fn new(
        server_host: String,
        server_port: u16,
        mount_point: String,
        password: String,
        audio_format: AudioFormat,
    ) -> Self {
        let client = IcecastSourceClient::new(
            server_host,
            server_port,
            mount_point,
            password,
            audio_format,
        );

        let (control_tx, control_rx) = mpsc::channel(16);

        Self {
            client,
            audio_rx: None,
            control_tx: Some(control_tx),
            control_rx: Some(control_rx),
            is_streaming: false,
        }
    }

    /// Connect audio input stream
    pub fn connect_audio_input(&mut self, audio_rx: mpsc::Receiver<Vec<u8>>) {
        self.audio_rx = Some(audio_rx);
        info!("🎵 Audio input connected to Icecast stream manager");
    }

    /// Start streaming
    pub async fn start_streaming(&mut self) -> Result<()> {
        if let Some(control_tx) = &self.control_tx {
            control_tx
                .send(StreamControl::Start)
                .await
                .context("Failed to send start command")?;
        }
        Ok(())
    }

    /// Stop streaming
    pub async fn stop_streaming(&mut self) -> Result<()> {
        if let Some(control_tx) = &self.control_tx {
            control_tx
                .send(StreamControl::Stop)
                .await
                .context("Failed to send stop command")?;
        }
        Ok(())
    }

    /// Update stream metadata
    pub async fn update_metadata(&mut self, title: String, artist: String) -> Result<()> {
        if let Some(control_tx) = &self.control_tx {
            control_tx
                .send(StreamControl::UpdateMetadata { title, artist })
                .await
                .context("Failed to send metadata update")?;
        }
        Ok(())
    }

    /// Run the streaming event loop
    pub async fn run(&mut self) -> Result<()> {
        info!("🚀 Starting Icecast stream manager...");

        let mut audio_rx = self
            .audio_rx
            .take()
            .ok_or_else(|| anyhow::anyhow!("Audio input not connected"))?;
        let mut control_rx = self
            .control_rx
            .take()
            .ok_or_else(|| anyhow::anyhow!("Control channel not available"))?;

        loop {
            tokio::select! {
                // Handle control commands
                Some(command) = control_rx.recv() => {
                    match command {
                        StreamControl::Start => {
                            info!("🎯 Starting Icecast stream...");
                            match self.client.connect().await {
                                Ok(()) => {
                                    info!("✅ Icecast stream started successfully");
                                    self.is_streaming = true;
                                }
                                Err(e) => {
                                    error!("❌ Failed to start Icecast stream: {}", e);
                                    return Err(e);
                                }
                            }
                        }

                        StreamControl::Stop => {
                            info!("🛑 Stopping Icecast stream...");
                            if let Err(e) = self.client.disconnect().await {
                                warn!("⚠️ Error during disconnect: {}", e);
                            }
                            self.is_streaming = false;
                        }

                        StreamControl::UpdateMetadata { title, artist } => {
                            info!("📝 Updating stream metadata: {} - {}", artist, title);
                            // Note: Metadata updates require separate admin connection
                            // This would be implemented with HTTP requests to admin interface
                        }
                    }
                }

                // Process audio data
                Some(audio_data) = audio_rx.recv() => {
                    if self.is_streaming && !audio_data.is_empty() {
                        if let Err(e) = self.client.send_audio_data(&audio_data).await {
                            error!("❌ Failed to send audio data: {}", e);

                            // Try to reconnect on error
                            warn!("🔄 Attempting to reconnect...");
                            if let Err(reconnect_err) = self.client.connect().await {
                                error!("❌ Reconnection failed: {}", reconnect_err);
                                self.is_streaming = false;
                                return Err(reconnect_err);
                            }
                        }
                    }
                }

                // Handle channel closures
                else => {
                    info!("📻 Icecast stream manager stopping...");
                    break;
                }
            }
        }

        // Cleanup
        if self.client.is_connected() {
            let _ = self.client.disconnect().await;
        }

        info!("🛑 Icecast stream manager stopped");
        Ok(())
    }

    /// Get streaming statistics
    pub fn get_stats(&self) -> IcecastStats {
        self.client.get_stats()
    }

    /// Check if streaming
    pub fn is_streaming(&self) -> bool {
        self.is_streaming && self.client.is_connected()
    }
}
