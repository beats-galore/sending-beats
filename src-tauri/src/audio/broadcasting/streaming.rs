use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub icecast_url: String,
    pub mount_point: String,
    pub username: String,
    pub password: String,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMetadata {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub genre: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStatus {
    pub is_connected: bool,
    pub is_streaming: bool,
    pub current_listeners: u32,
    pub peak_listeners: u32,
    pub stream_duration: u64,
    pub bitrate: u32,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct StreamManager {
    config: StreamConfig,
    status: Arc<Mutex<StreamStatus>>,
    client: Client,
    stream_sender: Option<mpsc::Sender<Vec<u8>>>,
}

impl StreamManager {
    pub fn new(config: StreamConfig) -> Self {
        let status = Arc::new(Mutex::new(StreamStatus {
            is_connected: false,
            is_streaming: false,
            current_listeners: 0,
            peak_listeners: 0,
            stream_duration: 0,
            bitrate: config.bitrate,
            error_message: None,
        }));

        let client = Client::new();

        Self {
            config,
            status,
            client,
            stream_sender: None,
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        let url = format!("{}/admin/stats", self.config.icecast_url);

        // Test connection to Icecast server
        let response = self
            .client
            .get(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .send()
            .await
            .context("Failed to connect to Icecast server")?;

        if response.status().is_success() {
            let mut status = self.status.lock().unwrap();
            status.is_connected = true;
            status.error_message = None;
            Ok(())
        } else {
            let mut status = self.status.lock().unwrap();
            status.error_message = Some(format!(
                "Icecast server returned status: {}",
                response.status()
            ));
            Err(anyhow::anyhow!(
                "Failed to authenticate with Icecast server"
            ))
        }
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        // Stop any active stream
        if let Some(sender) = &self.stream_sender {
            let _ = sender.send(vec![]).await; // Send empty data to signal stop
        }

        let mut status = self.status.lock().unwrap();
        status.is_connected = false;
        status.is_streaming = false;
        status.error_message = None;

        Ok(())
    }

    pub async fn start_stream(&mut self, _audio_data: Vec<u8>) -> Result<()> {
        if !self.status.lock().unwrap().is_connected {
            return Err(anyhow::anyhow!("Not connected to Icecast server"));
        }

        let stream_url = format!("{}/{}", self.config.icecast_url, self.config.mount_point);

        // Tokio channel for PCM data from async API/frontend
        let (tokio_pcm_tx, mut tokio_pcm_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(100);
        self.stream_sender = Some(tokio_pcm_tx);
        // Std channel for PCM data to encoding thread
        let (pcm_tx, pcm_rx) = std::sync::mpsc::channel::<Vec<u8>>();
        // Channel for MP3 data from encoder thread to async
        let (mp3_tx, mut mp3_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(100);
        // Channel for controlling the stream (stop signal)
        let (_control_tx, mut control_rx) = tokio::sync::mpsc::channel::<()>(1);

        // Forward PCM data from Tokio receiver to std sender (encoding thread)
        std::thread::spawn(move || {
            while let Some(pcm_data) = tokio_pcm_rx.blocking_recv() {
                if pcm_data.is_empty() {
                    break;
                }
                if pcm_tx.send(pcm_data).is_err() {
                    break;
                }
            }
        });

        // Encoder thread: PCM -> MP3
        let bitrate = self.config.bitrate;
        let sample_rate = self.config.sample_rate;
        let channels = self.config.channels;
        std::thread::spawn(move || {
            let mut encoder = AudioEncoder::new(bitrate, sample_rate, channels);
            while let Ok(pcm_data) = pcm_rx.recv() {
                if pcm_data.is_empty() {
                    break; // Stop signal
                }
                match encoder.encode_pcm_to_mp3(&pcm_data) {
                    Ok(mp3_data) => {
                        // Ignore send error if receiver is dropped
                        let _ = mp3_tx.blocking_send(mp3_data);
                    }
                    Err(_) => {
                        // Optionally: send error info
                        break;
                    }
                }
            }
        });

        // Start streaming task
        let client = self.client.clone();
        let config = self.config.clone();
        let status = self.status.clone();
        tokio::spawn(async move {
            let mut stream_duration = 0u64;
            loop {
                tokio::select! {
                    Some(mp3_data) = mp3_rx.recv() => {
                        // Send audio data to Icecast
                        let response = client
                            .post(&stream_url)
                            .basic_auth(&config.username, Some(&config.password))
                            .header("Content-Type", "audio/mpeg")
                            .header("Ice-Public", "1")
                            .header("Ice-Name", "Sweet Beats Radio")
                            .header("Ice-Description", "Live Radio Stream")
                            .header("Ice-Genre", "Electronic")
                            .body(mp3_data)
                            .send()
                            .await;
                        match response {
                            Ok(_) => {
                                stream_duration += 1;
                                let mut status = status.lock().unwrap();
                                status.is_streaming = true;
                                status.stream_duration = stream_duration;
                            }
                            Err(e) => {
                                let mut status = status.lock().unwrap();
                                status.error_message = Some(format!("Streaming error: {}", e));
                                break;
                            }
                        }
                    }
                    _ = control_rx.recv() => {
                        break;
                    }
                }
            }
            // Update status when streaming stops
            let mut status = status.lock().unwrap();
            status.is_streaming = false;
        });

        Ok(())
    }

    pub async fn stop_stream(&mut self) -> Result<()> {
        if let Some(sender) = &self.stream_sender {
            let _ = sender.send(vec![]).await; // Send stop signal
        }

        let mut status = self.status.lock().unwrap();
        status.is_streaming = false;

        Ok(())
    }

    pub async fn update_metadata(&self, metadata: StreamMetadata) -> Result<()> {
        if !self.status.lock().unwrap().is_connected {
            return Err(anyhow::anyhow!("Not connected to Icecast server"));
        }

        let metadata_url = format!("{}/admin/metadata", self.config.icecast_url);
        let mount = self.config.mount_point.clone();

        let metadata_body = format!(
            "mount={}&song={}",
            mount,
            urlencoding::encode(&format!("{} - {}", metadata.artist, metadata.title))
        );

        let response = self
            .client
            .post(&metadata_url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(metadata_body)
            .send()
            .await
            .context("Failed to update metadata")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to update metadata: {}",
                response.status()
            ));
        }

        Ok(())
    }

    pub async fn get_status(&self) -> StreamStatus {
        self.status.lock().unwrap().clone()
    }

    pub async fn get_listener_stats(&self) -> Result<(u32, u32)> {
        let stats_url = format!("{}/admin/stats", self.config.icecast_url);

        let response = self
            .client
            .get(&stats_url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .send()
            .await
            .context("Failed to get listener stats")?;

        if response.status().is_success() {
            let stats_text = response.text().await?;

            // Parse Icecast XML stats (simplified)
            let current_listeners = stats_text
                .lines()
                .find(|line| line.contains("currentlisteners"))
                .and_then(|line| line.split('>').nth(1)?.split('<').next())
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);

            let peak_listeners = stats_text
                .lines()
                .find(|line| line.contains("peaklisteners"))
                .and_then(|line| line.split('>').nth(1)?.split('<').next())
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);

            Ok((current_listeners, peak_listeners))
        } else {
            Err(anyhow::anyhow!(
                "Failed to get stats: {}",
                response.status()
            ))
        }
    }
}

// Audio encoding for the broadcast
//
// This used to hold a LAME handle it never called: `encode_pcm_to_mp3` decoded
// the bytes into a `Vec<i16>` it then discarded and returned the raw PCM
// unchanged — which the stream then labelled `Content-Type: audio/mpeg`. A
// listener was being sent PCM described as MP3.
//
// It now delegates to the encoder the recorder uses, which is a real one.
pub struct AudioEncoder {
    bitrate: u32,
    sample_rate: u32,
    channels: u16,
    lame: Option<crate::audio::recording::lame::Lame>,
}

impl AudioEncoder {
    pub fn new(bitrate: u32, sample_rate: u32, channels: u16) -> Self {
        let lame = crate::audio::recording::lame::Lame::new(sample_rate, channels, bitrate).ok();

        Self {
            bitrate,
            sample_rate,
            channels,
            lame,
        }
    }

    /// Encode interleaved 16-bit PCM to MP3
    ///
    /// Returns nothing when there is no encoder, rather than the input: sending
    /// PCM under an MP3 content type is worse than sending silence, because
    /// every listener's player treats it as a corrupt stream.
    pub fn encode_pcm_to_mp3(&mut self, pcm_data: &[u8]) -> Result<Vec<u8>> {
        let Some(lame) = self.lame.as_mut() else {
            return Ok(Vec::new());
        };

        let samples: Vec<f32> = pcm_data
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
            .collect();

        lame.encode(&samples)
    }
}
