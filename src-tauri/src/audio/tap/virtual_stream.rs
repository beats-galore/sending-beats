// Virtual audio stream and bridge implementations
//
// This module provides the bridge between application audio taps and the mixer system,
// implementing compatibility with the AudioInputStream interface and handling
// async/sync buffer management.

use anyhow::Result;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex as AsyncMutex;
use tracing::{info, warn};

/// Virtual audio input stream that bridges tap audio to mixer system
pub struct VirtualAudioInputStream {
    device_id: String,
    device_name: String,
    sample_rate: u32,
    channels: u16,
    bridge_buffer: Arc<AsyncMutex<Vec<f32>>>,
    effects_chain: Arc<AsyncMutex<crate::audio::effects::AudioEffectsChain>>,
}

impl VirtualAudioInputStream {
    pub fn new(
        device_id: String,
        device_name: String,
        sample_rate: u32,
        bridge_buffer: Arc<AsyncMutex<Vec<f32>>>,
    ) -> Self {
        let effects_chain = Arc::new(AsyncMutex::new(
            crate::audio::effects::AudioEffectsChain::new(sample_rate),
        ));

        Self {
            device_id,
            device_name,
            sample_rate,
            channels: 2, // Assume stereo for application audio
            bridge_buffer,
            effects_chain,
        }
    }

    /// Get samples from the bridge buffer (compatible with AudioInputStream interface)
    pub async fn get_samples(&self) -> Vec<f32> {
        if let Ok(mut buffer) = self.bridge_buffer.try_lock() {
            if buffer.is_empty() {
                return Vec::new();
            }

            // Drain all available samples
            let samples: Vec<f32> = buffer.drain(..).collect();
            samples
        } else {
            Vec::new()
        }
    }

    /// Process samples with effects (compatible with AudioInputStream interface)
    pub async fn process_with_effects(
        &self,
        channel: &crate::audio::types::AudioChannel,
    ) -> Vec<f32> {
        if let Ok(mut buffer) = self.bridge_buffer.try_lock() {
            if buffer.is_empty() {
                return Vec::new();
            }

            // Drain all available samples
            let mut samples: Vec<f32> = buffer.drain(..).collect();

            // Apply effects if enabled
            if channel.effects_enabled && !samples.is_empty() {
                if let Ok(mut effects) = self.effects_chain.try_lock() {
                    // Update effects parameters based on channel settings
                    effects.set_eq_gain(crate::audio::effects::EQBand::Low, channel.eq_low_gain);
                    effects.set_eq_gain(crate::audio::effects::EQBand::Mid, channel.eq_mid_gain);
                    effects.set_eq_gain(crate::audio::effects::EQBand::High, channel.eq_high_gain);

                    if channel.comp_enabled {
                        effects.set_compressor_params(
                            channel.comp_threshold,
                            channel.comp_ratio,
                            channel.comp_attack,
                            channel.comp_release,
                        );
                    }

                    if channel.limiter_enabled {
                        effects.set_limiter_threshold(channel.limiter_threshold);
                    }

                    // Process samples through effects chain
                    effects.process(&mut samples);
                }
            }

            // Apply channel-specific gain and mute
            if !channel.muted && channel.gain > 0.0 {
                for sample in samples.iter_mut() {
                    *sample *= channel.gain;
                }
            } else {
                samples.fill(0.0);
            }

            samples
        } else {
            Vec::new()
        }
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }
}

/// Bridge adapter that converts VirtualAudioInputStream to AudioInputStream interface
pub struct ApplicationAudioInputBridge {
    device_id: String,
    device_name: String,
    sample_rate: u32,
    channels: u16,
    audio_buffer: Arc<AsyncMutex<Vec<f32>>>, // Source buffer from tap bridge
    sync_buffer: Arc<StdMutex<Vec<f32>>>,    // Sync buffer for mixer compatibility
    effects_chain: Arc<StdMutex<crate::audio::effects::AudioEffectsChain>>,
    adaptive_chunk_size: usize,
}

impl ApplicationAudioInputBridge {
    pub fn new(
        device_id: String,
        device_name: String,
        sample_rate: u32,
        audio_buffer: Arc<AsyncMutex<Vec<f32>>>,
    ) -> Result<Self> {
        let sync_buffer = Arc::new(StdMutex::new(Vec::new()));
        let effects_chain = Arc::new(StdMutex::new(
            crate::audio::effects::AudioEffectsChain::new(sample_rate),
        ));

        // Calculate optimal chunk size (same as AudioInputStream)
        let optimal_chunk_size = (sample_rate as f32 * 0.005) as usize; // 5ms default

        Ok(Self {
            device_id,
            device_name,
            sample_rate,
            channels: 2, // Assume stereo for application audio
            audio_buffer,
            sync_buffer,
            effects_chain,
            adaptive_chunk_size: optimal_chunk_size.max(64).min(1024),
        })
    }

    /// Synchronously transfer samples from async buffer to sync buffer
    /// This should be called periodically to keep the sync buffer updated
    pub fn sync_transfer_samples(&self) {
        // Use try_lock to avoid blocking - if async buffer is locked, skip this transfer
        if let Ok(mut async_buffer) = self.audio_buffer.try_lock() {
            if !async_buffer.is_empty() {
                // Transfer samples from async buffer to sync buffer
                let samples: Vec<f32> = async_buffer.drain(..).collect();

                if let Ok(mut sync_buffer) = self.sync_buffer.try_lock() {
                    sync_buffer.extend_from_slice(&samples);

                    // Prevent buffer overflow - same logic as regular input streams
                    let max_buffer_size = 48000; // 1 second at 48kHz
                    if sync_buffer.len() > max_buffer_size * 2 {
                        let keep_size = max_buffer_size;
                        let buffer_len = sync_buffer.len();
                        let new_buffer = sync_buffer.split_off(buffer_len - keep_size);
                        *sync_buffer = new_buffer;
                    }
                }
            }
        }
    }

    /// Get samples (compatible with AudioInputStream interface)
    pub fn get_samples(&self) -> Vec<f32> {
        // First, transfer any new samples from async buffer
        self.sync_transfer_samples();

        // Then get samples from sync buffer (same as AudioInputStream)
        if let Ok(mut buffer) = self.sync_buffer.try_lock() {
            if buffer.is_empty() {
                return Vec::new();
            }

            // Process ALL available samples to prevent buffer buildup
            let samples: Vec<f32> = buffer.drain(..).collect();
            samples
        } else {
            Vec::new()
        }
    }

    /// Process samples with effects (compatible with AudioInputStream interface)
    pub fn process_with_effects(&self, channel: &crate::audio::types::AudioChannel) -> Vec<f32> {
        // First, transfer any new samples from async buffer
        self.sync_transfer_samples();

        if let Ok(mut buffer) = self.sync_buffer.try_lock() {
            if buffer.is_empty() {
                return Vec::new();
            }

            // Drain all available samples
            let mut samples: Vec<f32> = buffer.drain(..).collect();

            // Apply effects if enabled
            if channel.effects_enabled && !samples.is_empty() {
                if let Ok(mut effects) = self.effects_chain.try_lock() {
                    // Update effects parameters based on channel settings
                    effects.set_eq_gain(crate::audio::effects::EQBand::Low, channel.eq_low_gain);
                    effects.set_eq_gain(crate::audio::effects::EQBand::Mid, channel.eq_mid_gain);
                    effects.set_eq_gain(crate::audio::effects::EQBand::High, channel.eq_high_gain);

                    if channel.comp_enabled {
                        effects.set_compressor_params(
                            channel.comp_threshold,
                            channel.comp_ratio,
                            channel.comp_attack,
                            channel.comp_release,
                        );
                    }

                    if channel.limiter_enabled {
                        effects.set_limiter_threshold(channel.limiter_threshold);
                    }

                    // Process samples through effects chain
                    effects.process(&mut samples);
                }
            }

            // Apply channel-specific gain and mute
            if !channel.muted && channel.gain > 0.0 {
                for sample in samples.iter_mut() {
                    *sample *= channel.gain;
                }
            } else {
                samples.fill(0.0);
            }

            samples
        } else {
            Vec::new()
        }
    }

    /// Set adaptive chunk size (compatible with AudioInputStream interface)
    pub fn set_adaptive_chunk_size(&mut self, hardware_buffer_size: usize) {
        let adaptive_size = if hardware_buffer_size > 32 && hardware_buffer_size <= 2048 {
            hardware_buffer_size
        } else {
            (self.sample_rate as f32 * 0.005) as usize
        };

        self.adaptive_chunk_size = adaptive_size;
        info!(
            "🔧 ADAPTIVE BUFFER: Set chunk size to {} samples for app device {}",
            self.adaptive_chunk_size, self.device_id
        );
    }

    // Getters (compatible with AudioInputStream interface)
    pub fn device_id(&self) -> &str {
        &self.device_id
    }
    pub fn device_name(&self) -> &str {
        &self.device_name
    }
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

/// Registry for virtual input streams - changed to HashSet to avoid RTRB Send+Sync issues
pub fn get_virtual_input_registry() -> &'static StdMutex<std::collections::HashSet<String>> {
    use std::sync::LazyLock;
    // Changed to HashSet<String> to avoid RTRB Send+Sync issues
    static VIRTUAL_INPUT_REGISTRY: LazyLock<
        StdMutex<std::collections::HashSet<String>>,
    > = LazyLock::new(|| StdMutex::new(std::collections::HashSet::new()));
    &VIRTUAL_INPUT_REGISTRY
}
