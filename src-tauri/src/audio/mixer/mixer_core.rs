// Core mixer implementation and coordination
//
// This module provides the remaining core mixer functionality that doesn't
// fit into the specialized modules, including device health monitoring,
// error reporting, and integration methods.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use super::super::devices::DeviceHealth;
use super::stream_management::{AudioInputStream, AudioOutputStream, StreamInfo};
use super::types::VirtualMixer;
use crate::audio::types::{AudioChannel, MixerConfig};

// Helper structure for processing thread (using command channel architecture)
#[derive(Debug)]
pub struct VirtualMixerHandle {
    pub audio_command_tx:
        tokio::sync::mpsc::Sender<crate::audio::mixer::stream_management::AudioCommand>,
    #[cfg(target_os = "macos")]
    pub coreaudio_stream:
        Arc<Mutex<Option<crate::audio::devices::coreaudio_stream::CoreAudioOutputStream>>>,
    pub channel_levels: Arc<Mutex<std::collections::HashMap<u32, (f32, f32, f32, f32)>>>,
    pub config: Arc<std::sync::Mutex<MixerConfig>>,
}

impl VirtualMixerHandle {
    /// Get samples from all active input streams with effects processing (using command channel)
    pub async fn collect_input_samples_with_effects(
        &self,
        channels: &[AudioChannel],
    ) -> HashMap<String, Vec<f32>> {
        let mut samples = HashMap::new();

        // Debug removed to reduce log spam

        // Send GetSamples command to IsolatedAudioManager for each active channel
        for channel in channels {
            if let Some(device_id) = &channel.input_device_id {
                // Use command channel to request samples from lock-free RTRB queues
                let (response_tx, response_rx) = tokio::sync::oneshot::channel();

                let command = crate::audio::mixer::stream_management::AudioCommand::GetSamples {
                    device_id: device_id.clone(),
                    channel_config: channel.clone(),
                    response_tx,
                };

                // Send command to isolated audio thread
                if let Err(_) = self.audio_command_tx.send(command).await {
                    continue; // Skip if command channel is closed
                }

                // Receive processed samples from lock-free pipeline
                if let Ok(stream_samples) = response_rx.await {
                    if !stream_samples.is_empty() {
                        samples.insert(device_id.clone(), stream_samples);
                    }
                }
            }
        }

        samples
    }

}

impl VirtualMixer {
    /// Check if the mixer is currently running
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::Relaxed)
    }
}
