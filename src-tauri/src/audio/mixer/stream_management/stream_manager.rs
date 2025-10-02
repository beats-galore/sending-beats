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
        input_notifier: Arc<Notify>,
    ) -> Result<()> {
        info!(
            "🎤 Creating CoreAudio input stream for device '{}' (ID: {}, CH: {})",
            device_id, coreaudio_device_id, channels
        );

        let mut coreaudio_input_stream =
            crate::audio::devices::CoreAudioInputStream::new_with_rtrb_producer(
                coreaudio_device_id,
                device_name.clone(),
                channels,
                producer, // Use producer provided by IsolatedAudioManager
                input_notifier,
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
        output_notifier: Arc<Notify>,
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
                output_notifier,               // **OUTPUT NOTIFIER INTEGRATION**
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
    pub fn has_input_stream(&self, device_id: &str) -> bool {
        self.coreaudio_input_streams.contains_key(device_id)
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
