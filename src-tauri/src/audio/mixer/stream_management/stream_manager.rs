use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use tokio::sync::Notify;

// Lock-free audio buffer imports
use rtrb::Producer;

#[derive(Debug, Clone)]
pub struct AudioMetrics {
    pub input_streams: usize,
    pub output_streams: usize,
    pub total_samples_processed: u64,
    pub buffer_underruns: u32,
    pub average_latency_ms: f32,
}

pub struct StreamManager {
    #[cfg(target_os = "macos")]
    coreaudio_streams: HashMap<String, crate::audio::devices::CoreAudioOutputStream>,
    #[cfg(target_os = "macos")]
    coreaudio_input_streams: HashMap<String, crate::audio::devices::CoreAudioInputStream>,
    #[cfg(target_os = "macos")]
    screencapture_streams: HashMap<String, crate::audio::screencapture::ScreenCaptureAudioStream>,
}

impl std::fmt::Debug for StreamManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamManager").finish()
    }
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            coreaudio_streams: HashMap::new(),
            #[cfg(target_os = "macos")]
            coreaudio_input_streams: HashMap::new(),
            #[cfg(target_os = "macos")]
            screencapture_streams: HashMap::new(),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn add_coreaudio_input_stream(
        &mut self,
        device_id: String,
        coreaudio_device_id: coreaudio_sys::AudioDeviceID,
        device_name: String,
        channels: u16,
        producer: Producer<f32>,
    ) -> Result<()> {
        info!(
            "🎤 Creating CoreAudio input stream for device '{}' (ID: {}, CH: {})",
            device_id, coreaudio_device_id, channels
        );
        info!(
            "🔍 STREAM_MANAGER: Currently active input streams: {:?}",
            self.coreaudio_input_streams.keys().collect::<Vec<_>>()
        );

        // Check if stream already exists for this device and remove it first
        if self.coreaudio_input_streams.contains_key(&device_id) {
            info!(
                "🔄 Removing existing stream for device '{}' before adding new one",
                device_id
            );
            if let Some(mut old_stream) = self.coreaudio_input_streams.remove(&device_id) {
                let _ = old_stream.stop();
                drop(old_stream);
                // Wait for CoreAudio cleanup to complete (avoid -536870186 error)
                std::thread::sleep(std::time::Duration::from_millis(150));
                info!("✅ Old stream cleanup complete for device '{}'", device_id);
            }
        }

        let mut coreaudio_input_stream =
            crate::audio::devices::CoreAudioInputStream::new_with_rtrb_producer(
                coreaudio_device_id,
                device_name.clone(),
                channels,
                producer, // Use producer provided by IsolatedAudioManager
            )?;

        // Start the CoreAudio input stream
        coreaudio_input_stream.start()?;

        // Store the CoreAudio input stream to prevent it from being dropped
        self.coreaudio_input_streams
            .insert(device_id.clone(), coreaudio_input_stream);

        info!(
            "✅ CoreAudio input stream created and started for device '{}'",
            device_id
        );
        Ok(())
    }

    /// Remove a stream by device ID
    pub fn remove_stream(&mut self, device_id: &str) -> bool {
        let mut removed = false;

        // Try to remove CoreAudio output stream on macOS
        #[cfg(target_os = "macos")]
        {
            if let Some(mut coreaudio_stream) = self.coreaudio_streams.remove(device_id) {
                println!(
                    "Stopping and removing CoreAudio output stream for device: {}",
                    device_id
                );
                // Explicitly stop the CoreAudio stream before dropping
                if let Err(e) = coreaudio_stream.stop() {
                    eprintln!(
                        "Warning: Failed to stop CoreAudio output stream {}: {}",
                        device_id, e
                    );
                }
                drop(coreaudio_stream);
                removed = true;
            }
        }

        // Try to remove CoreAudio input stream on macOS
        #[cfg(target_os = "macos")]
        {
            if let Some(mut coreaudio_input_stream) = self.coreaudio_input_streams.remove(device_id)
            {
                println!(
                    "Stopping and removing CoreAudio input stream for device: {}",
                    device_id
                );
                // Explicitly stop the CoreAudio input stream before dropping
                if let Err(e) = coreaudio_input_stream.stop() {
                    eprintln!(
                        "Warning: Failed to stop CoreAudio input stream {}: {}",
                        device_id, e
                    );
                }
                drop(coreaudio_input_stream);
                removed = true;
            }
        }

        // Try to remove ScreenCaptureKit stream on macOS
        #[cfg(target_os = "macos")]
        {
            if let Some(mut screencapture_stream) = self.screencapture_streams.remove(device_id) {
                println!(
                    "Stopping and removing ScreenCaptureKit stream for device: {}",
                    device_id
                );
                if let Err(e) = screencapture_stream.stop() {
                    eprintln!(
                        "Warning: Failed to stop ScreenCaptureKit stream {}: {}",
                        device_id, e
                    );
                }
                drop(screencapture_stream);
                removed = true;
            }
        }

        if !removed {
            println!("Stream not found for removal: {}", device_id);
        }

        removed
    }

    /// Add CoreAudio output stream with SPMC reader and queue tracker for advanced monitoring
    #[cfg(target_os = "macos")]
    pub fn add_coreaudio_output_stream_with_tracker(
        &mut self,
        device_id: String,
        coreaudio_device: crate::audio::types::CoreAudioDevice,
        rtrb_consumer: rtrb::Consumer<f32>,
        queue_tracker: AtomicQueueTracker,
    ) -> Result<()> {
        info!(
            "🔊 Creating CoreAudio output stream with queue tracker for device '{}' (ID: {})",
            device_id, coreaudio_device.device_id
        );

        // **RTRB INTEGRATION**: Create CoreAudio stream with RTRB consumer AND output notifier
        let mut coreaudio_stream =
            crate::audio::devices::CoreAudioOutputStream::new_with_rtrb_consumer_and_notifier(
                coreaudio_device.device_id,    // AudioDeviceID (u32)
                coreaudio_device.name.clone(), // String
                coreaudio_device.channels,     // Use actual device channel count (dynamic)
                rtrb_consumer,                 // **RTRB CONSUMER INTEGRATION**
                queue_tracker,                 // **QUEUE TRACKING INTEGRATION**
            )?;

        // Start the CoreAudio stream
        coreaudio_stream.start()?;

        // Store the CoreAudio stream to prevent it from being dropped
        self.coreaudio_streams
            .insert(device_id.clone(), coreaudio_stream);

        info!(
            "🎵 CoreAudio stream started with SPMC queue integration and queue tracking for device '{}'",
            device_id
        );

        info!(
            "✅ CoreAudio output stream with queue tracker created and started for device '{}'",
            device_id
        );
        Ok(())
    }
    #[cfg(target_os = "macos")]
    pub fn add_screencapture_stream(
        &mut self,
        device_id: String,
        pid: i32,
        device_name: String,
        producer: Producer<f32>,
    ) -> Result<f64> {
        info!(
            "📺 Creating ScreenCaptureKit stream for device '{}' (PID: {})",
            device_id, pid
        );

        let mut screencapture_stream =
            crate::audio::screencapture::ScreenCaptureAudioStream::new(pid, device_name.clone());

        let sample_rate = screencapture_stream.start(producer)?;

        self.screencapture_streams
            .insert(device_id.clone(), screencapture_stream);

        info!(
            "✅ ScreenCaptureKit stream created and started for device '{}' at {} Hz",
            device_id, sample_rate
        );
        Ok(sample_rate)
    }

    #[cfg(target_os = "macos")]
    pub fn has_input_stream(&self, device_id: &str) -> bool {
        self.coreaudio_input_streams.contains_key(device_id)
            || self.screencapture_streams.contains_key(device_id)
    }
    #[cfg(target_os = "macos")]
    pub fn has_output_stream(&self, device_id: &str) -> bool {
        self.coreaudio_streams.contains_key(device_id)
    }

    /// Update hardware buffer size for a CoreAudio output stream
    #[cfg(target_os = "macos")]
    pub fn update_coreaudio_output_buffer_size(
        &self,
        device_id: &str,
        target_frames: u32,
    ) -> Result<()> {
        if let Some(stream) = self.coreaudio_streams.get(device_id) {
            stream.set_dynamic_buffer_size(target_frames)?;
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "CoreAudio stream '{}' not found",
                device_id
            ))
        }
    }
}
