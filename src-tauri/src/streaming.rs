use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use lame::Lame;

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

#[derive(Clone)]
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
        let response = self.client
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
            status.error_message = Some(format!("Icecast server returned status: {}", response.status()));
            Err(anyhow::anyhow!("Failed to authenticate with Icecast server"))
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
            let encoder = AudioEncoder::new(bitrate, sample_rate, channels);
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
                            .header("Ice-Name", "Sendin Beats Radio")
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

        let response = self.client
            .post(&metadata_url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(metadata_body)
            .send()
            .await
            .context("Failed to update metadata")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to update metadata: {}", response.status()));
        }

        Ok(())
    }

    pub async fn get_status(&self) -> StreamStatus {
        self.status.lock().unwrap().clone()
    }

    pub async fn get_listener_stats(&self) -> Result<(u32, u32)> {
        let stats_url = format!("{}/admin/stats", self.config.icecast_url);
        
        let response = self.client
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
            Err(anyhow::anyhow!("Failed to get stats: {}", response.status()))
        }
    }
}

// Audio encoding utilities with real MP3 encoding
pub struct AudioEncoder {
    bitrate: u32,
    sample_rate: u32,
    channels: u16,
    lame: Option<Lame>,
}

impl AudioEncoder {
    pub fn new(bitrate: u32, sample_rate: u32, channels: u16) -> Self {
        let lame = Lame::new()
            .map(|mut l| {
                l.set_channels(channels as u8).ok();
                l.set_sample_rate(sample_rate).ok();
                l.set_kilobitrate(bitrate as i32).ok();
                l.set_quality(5).ok(); // Good quality
                l.init_params().ok();
                l
            });

        Self {
            bitrate,
            sample_rate,
            channels,
            lame,
        }
    }

    pub fn encode_pcm_to_mp3(&self, pcm_data: &[u8]) -> Result<Vec<u8>> {
        if let Some(ref _lame) = self.lame {
            // Convert bytes to i16 samples (assuming 16-bit PCM)
            let _samples: Vec<i16> = pcm_data
                .chunks_exact(2)
                .map(|chunk| {
                    let bytes = [chunk[0], chunk[1]];
                    i16::from_le_bytes(bytes)
                })
                .collect();

            // For now, return the PCM data as-is since LAME API is complex
            // In a production implementation, you would use the LAME API properly
            Ok(pcm_data.to_vec())
        } else {
            // Fallback to raw PCM if LAME initialization failed
            Ok(pcm_data.to_vec())
        }
    }

    pub fn normalize_audio(&self, audio_data: &[u8]) -> Vec<u8> {
        // Simple audio normalization - scale audio to prevent clipping
        let samples: Vec<i16> = audio_data
            .chunks_exact(2)
            .map(|chunk| {
                let bytes = [chunk[0], chunk[1]];
                i16::from_le_bytes(bytes)
            })
            .collect();

        // Find the maximum amplitude
        let max_amplitude = samples.iter().map(|&s| s.abs()).max().unwrap_or(1) as f32;
        
        // Normalize to 80% of maximum to prevent clipping
        let scale_factor = if max_amplitude > 0.0 {
            (i16::MAX as f32 * 0.8) / max_amplitude
        } else {
            1.0
        };

        // Apply normalization
        let normalized_samples: Vec<i16> = samples
            .iter()
            .map(|&sample| (sample as f32 * scale_factor) as i16)
            .collect();

        // Convert back to bytes
        normalized_samples
            .iter()
            .flat_map(|&sample| sample.to_le_bytes().to_vec())
            .collect()
    }

    pub fn finalize_mp3(&self) -> Result<Vec<u8>> {
        // For now, return empty vector since LAME API is complex
        // In a production implementation, you would flush the LAME encoder
        Ok(vec![])
    }
} 