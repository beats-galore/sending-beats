use anyhow::Result;
use std::sync::Arc;
use tokio::sync::OnceCell;

use super::config::StreamingServiceConfig;
use super::icecast_source::AudioCodec;
use super::manager::StreamingService;
use super::types::StreamingServiceStatus;
use crate::audio::VirtualMixer;

/// Global streaming service instance
static STREAMING_SERVICE: OnceCell<Arc<StreamingService>> = OnceCell::const_new();

/// Get or create the global streaming service
pub async fn get_streaming_service() -> Arc<StreamingService> {
    STREAMING_SERVICE
        .get_or_init(|| async { Arc::new(StreamingService::new()) })
        .await
        .clone()
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

/// Start streaming with an RTRB consumer from the audio pipeline
pub async fn start_streaming_with_consumer(
    config: StreamingServiceConfig,
    rtrb_consumer: rtrb::Consumer<f32>,
) -> Result<()> {
    let service = get_streaming_service().await;
    service
        .start_streaming_with_consumer(config, rtrb_consumer)
        .await
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
pub fn create_stream_bitrate_preset(
    bitrate: u32,
    codec: AudioCodec,
) -> Result<StreamingServiceConfig> {
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
