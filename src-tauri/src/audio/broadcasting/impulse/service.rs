// Starting, stopping and reporting on an Impulse broadcast.
//
// The Icecast service is built around a connection it holds and monitors. There
// is nothing to hold here, so there is nothing to monitor: a broadcast is on air
// while segments are landing, and the far end says so in its reply to every one
// of them. That answer is more honest than a socket that is still open —
// a connection can be up while nobody is listening to anything, but a segment
// acknowledged on air was genuinely put in front of listeners.

use anyhow::Result;
use colored::*;
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell, RwLock};
use tokio::time::Instant;
use tracing::info;

use super::send_loop::{ImpulseSendLoop, ImpulseStats};
use super::uploader::ImpulseUploader;

/// Everything needed to put a station on air over Impulse
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImpulseConfig {
    /// Where the ingest worker answers, scheme included
    pub endpoint_url: String,
    /// Names the station on the other end
    pub station_slug: String,
    /// Read from the keychain at the moment of going live, never stored here
    #[serde(skip_serializing)]
    pub token: String,
    pub segment_ms: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub bitrate_kbps: u32,
}

/// The Impulse transmitter, of which there is one
#[derive(Debug, Default)]
pub struct ImpulseService {
    send_loop: Mutex<Option<ImpulseSendLoop>>,
    config: RwLock<Option<ImpulseConfig>>,
    started_at: Mutex<Option<Instant>>,
}

impl ImpulseService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Go on air, sending the mix as segments
    ///
    /// The first segment is what proves this works, and it is four seconds away
    /// — so unlike a socket, there is nothing to fail here that would tell the
    /// caller the station is unreachable. What can be checked up front is
    /// checked up front, and the rest surfaces in the status.
    pub async fn start(&self, config: ImpulseConfig, consumer: rtrb::Consumer<f32>) -> Result<()> {
        let mut running = self.send_loop.lock().await;

        if running.is_some() {
            return Err(anyhow::anyhow!("This stream is already on air"));
        }

        if config.station_slug.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "This station has no slug, so there is nothing on the other end to send to"
            ));
        }

        let uploader =
            ImpulseUploader::new(&config.endpoint_url, &config.station_slug, &config.token)?;

        let send_loop = ImpulseSendLoop::start(
            uploader,
            consumer,
            config.sample_rate,
            config.channels,
            config.bitrate_kbps,
            config.segment_ms,
        )?;

        info!(
            "✅ {}: On air as '{}' — {} ms segments to {}",
            "IMPULSE".on_purple().white(),
            config.station_slug,
            config.segment_ms,
            config.endpoint_url
        );

        *running = Some(send_loop);
        *self.config.write().await = Some(config);
        *self.started_at.lock().await = Some(Instant::now());

        Ok(())
    }

    /// Come off air, finishing the queue and signing off
    pub async fn stop(&self) -> Result<()> {
        let running = self.send_loop.lock().await.take();

        if let Some(send_loop) = running {
            send_loop.stop().await;
        }

        *self.started_at.lock().await = None;

        info!("🛑 {}: Off air", "IMPULSE".on_purple().white());
        Ok(())
    }

    pub async fn is_running(&self) -> bool {
        self.send_loop.lock().await.is_some()
    }

    pub async fn config(&self) -> Option<ImpulseConfig> {
        self.config.read().await.clone()
    }

    pub async fn uptime_seconds(&self) -> u64 {
        self.started_at
            .lock()
            .await
            .map(|start| start.elapsed().as_secs())
            .unwrap_or(0)
    }

    pub async fn stats(&self) -> Option<ImpulseStats> {
        match self.send_loop.lock().await.as_ref() {
            Some(send_loop) => Some(send_loop.stats().await),
            None => None,
        }
    }
}

static IMPULSE_SERVICE: OnceCell<Arc<ImpulseService>> = OnceCell::const_new();

pub async fn get_impulse_service() -> Arc<ImpulseService> {
    IMPULSE_SERVICE
        .get_or_init(|| async { Arc::new(ImpulseService::new()) })
        .await
        .clone()
}
