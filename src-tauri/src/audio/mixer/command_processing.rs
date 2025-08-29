// Command handling and inter-component communication
//
// This module manages command processing, communication channels, and
// coordination between different parts of the mixer system. It provides
// the messaging infrastructure for real-time mixer control.

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, warn, error};

use super::super::types::{AudioChannel, MixerCommand};
use super::types::VirtualMixer;

impl VirtualMixer {
    /// Send a command to the mixer for processing
    pub async fn send_command(&self, command: MixerCommand) -> Result<()> {
        self.command_tx.send(command).await
            .map_err(|e| anyhow::anyhow!("Failed to send mixer command: {}", e))?;
        Ok(())
    }

    /// Process pending commands from the command queue
    pub async fn process_commands(&mut self) -> Result<()> {
        let mut command_rx = self.command_rx.lock().await;
        
        // Process all available commands without blocking
        while let Ok(command) = command_rx.try_recv() {
            if let Err(e) = self.handle_command(command).await {
                error!("Failed to process mixer command: {}", e);
            }
        }
        
        Ok(())
    }

    /// Handle a single mixer command
    async fn handle_command(&mut self, command: MixerCommand) -> Result<()> {
        match command {
            MixerCommand::AddChannel(channel) => {
                self.add_channel(channel).await?;
            }
            MixerCommand::UpdateChannel { channel_id, channel } => {
                self.update_channel(channel_id, channel).await?;
            }
            MixerCommand::RemoveChannel(channel_id) => {
                self.remove_channel(channel_id).await?;
            }
            MixerCommand::SetMasterVolume(volume) => {
                self.set_master_volume(volume).await?;
            }
            MixerCommand::SetChannelVolume { channel_id, volume } => {
                self.set_channel_volume(channel_id, volume).await?;
            }
            MixerCommand::MuteChannel { channel_id, muted } => {
                self.mute_channel(channel_id, muted).await?;
            }
            MixerCommand::SoloChannel { channel_id, solo } => {
                self.solo_channel(channel_id, solo).await?;
            }
            MixerCommand::UpdateConfig(config) => {
                self.update_config(config).await?;
            }
            MixerCommand::Stop => {
                info!("Received stop command");
                self.stop().await?;
            }
        }
        
        Ok(())
    }

    /// Add a new audio channel to the mixer
    pub async fn add_channel(&mut self, channel: AudioChannel) -> Result<()> {
        // Validate channel
        super::validation::validate_channel_id(channel.id)?;
        
        // Update shared configuration
        if let Ok(mut shared_config) = self.shared_config.lock() {
            // Add channel to configuration if not already present
            if !shared_config.channels.iter().any(|c| c.id == channel.id) {
                shared_config.channels.push(channel.clone());
                info!("Added channel {} to mixer", channel.id);
            } else {
                warn!("Channel {} already exists, updating instead", channel.id);
                // Update existing channel
                if let Some(existing_channel) = shared_config.channels.iter_mut().find(|c| c.id == channel.id) {
                    *existing_channel = channel;
                }
            }
        }
        
        Ok(())
    }

    /// Update an existing audio channel
    pub async fn update_channel(&mut self, channel_id: u32, updated_channel: AudioChannel) -> Result<()> {
        super::validation::validate_channel_id(channel_id)?;
        
        if updated_channel.id != channel_id {
            return Err(anyhow::anyhow!(
                "Channel ID mismatch: expected {}, got {}", 
                channel_id, updated_channel.id
            ));
        }
        
        // Update shared configuration
        if let Ok(mut shared_config) = self.shared_config.lock() {
            if let Some(existing_channel) = shared_config.channels.iter_mut().find(|c| c.id == channel_id) {
                *existing_channel = updated_channel;
                info!("Updated channel {}", channel_id);
            } else {
                return Err(anyhow::anyhow!("Channel {} not found for update", channel_id));
            }
        }
        
        Ok(())
    }

    /// Remove an audio channel from the mixer
    async fn remove_channel(&mut self, channel_id: u32) -> Result<()> {
        super::validation::validate_channel_id(channel_id)?;
        
        // Update shared configuration
        if let Ok(mut shared_config) = self.shared_config.lock() {
            let initial_len = shared_config.channels.len();
            shared_config.channels.retain(|c| c.id != channel_id);
            
            if shared_config.channels.len() < initial_len {
                info!("Removed channel {}", channel_id);
            } else {
                warn!("Channel {} not found for removal", channel_id);
            }
        }
        
        // Clear channel levels
        {
            let mut levels = self.channel_levels.lock().await;
            levels.remove(&channel_id);
            
            let mut levels_cache = self.channel_levels_cache.lock().await;
            levels_cache.remove(&channel_id);
        }
        
        Ok(())
    }

    /// Set master volume
    async fn set_master_volume(&mut self, volume: f32) -> Result<()> {
        super::validation::SecurityUtils::validate_safe_float(volume, "master volume")?;
        
        if volume < 0.0 || volume > 2.0 {
            return Err(anyhow::anyhow!("Master volume must be between 0.0 and 2.0, got {}", volume));
        }
        
        if let Ok(mut shared_config) = self.shared_config.lock() {
            shared_config.master_volume = volume;
            info!("Set master volume to {:.2}", volume);
        }
        
        Ok(())
    }

    /// Set channel volume
    async fn set_channel_volume(&mut self, channel_id: u32, volume: f32) -> Result<()> {
        super::validation::validate_channel_id(channel_id)?;
        super::validation::SecurityUtils::validate_safe_float(volume, "channel volume")?;
        
        if volume < 0.0 || volume > 2.0 {
            return Err(anyhow::anyhow!("Channel volume must be between 0.0 and 2.0, got {}", volume));
        }
        
        if let Ok(mut shared_config) = self.shared_config.lock() {
            if let Some(channel) = shared_config.channels.iter_mut().find(|c| c.id == channel_id) {
                channel.volume = volume;
                info!("Set channel {} volume to {:.2}", channel_id, volume);
            } else {
                return Err(anyhow::anyhow!("Channel {} not found for volume update", channel_id));
            }
        }
        
        Ok(())
    }

    /// Mute/unmute a channel
    async fn mute_channel(&mut self, channel_id: u32, muted: bool) -> Result<()> {
        super::validation::validate_channel_id(channel_id)?;
        
        if let Ok(mut shared_config) = self.shared_config.lock() {
            if let Some(channel) = shared_config.channels.iter_mut().find(|c| c.id == channel_id) {
                channel.muted = muted;
                info!("Channel {} {}", channel_id, if muted { "muted" } else { "unmuted" });
            } else {
                return Err(anyhow::anyhow!("Channel {} not found for mute update", channel_id));
            }
        }
        
        Ok(())
    }

    /// Solo/unsolo a channel
    async fn solo_channel(&mut self, channel_id: u32, solo: bool) -> Result<()> {
        super::validation::validate_channel_id(channel_id)?;
        
        if let Ok(mut shared_config) = self.shared_config.lock() {
            if let Some(channel) = shared_config.channels.iter_mut().find(|c| c.id == channel_id) {
                channel.solo = solo;
                info!("Channel {} {}", channel_id, if solo { "soloed" } else { "unsoloed" });
                
                // If this channel is being soloed, other channels should be muted in the mix
                // This is handled in the audio processing logic
            } else {
                return Err(anyhow::anyhow!("Channel {} not found for solo update", channel_id));
            }
        }
        
        Ok(())
    }

    /// Update mixer configuration
    async fn update_config(&mut self, config: crate::audio::types::MixerConfig) -> Result<()> {
        super::validation::validate_config(&config)?;
        
        if let Ok(mut shared_config) = self.shared_config.lock() {
            let old_sample_rate = shared_config.sample_rate;
            *shared_config = config.clone();
            
            // Update audio clock if sample rate changed
            if config.sample_rate != old_sample_rate {
                let mut audio_clock = self.audio_clock.lock().await;
                audio_clock.set_sample_rate(config.sample_rate);
                info!("Updated mixer configuration, sample rate: {} -> {}", 
                      old_sample_rate, config.sample_rate);
            }
        }
        
        self.config = config;
        Ok(())
    }

    /// Get a receiver for audio output (for streaming integration)
    pub fn get_audio_output_receiver(&self) -> tokio::sync::broadcast::Receiver<Vec<f32>> {
        self.audio_output_broadcast_tx.subscribe()
    }

    /// Create a new audio receiver for streaming
    pub async fn create_streaming_audio_receiver(&self) -> mpsc::Receiver<Vec<f32>> {
        let (tx, rx) = mpsc::channel(100);
        
        // Clone the broadcast sender to forward audio data
        let broadcast_rx = self.audio_output_broadcast_tx.subscribe();
        
        // Spawn a task to forward broadcast messages to the new receiver
        tokio::spawn(async move {
            let mut broadcast_rx = broadcast_rx;
            while let Ok(audio_data) = broadcast_rx.recv().await {
                if tx.send(audio_data).await.is_err() {
                    // Receiver dropped, exit forwarding task
                    break;
                }
            }
        });
        
        rx
    }
}

/// Command queue utilities
pub struct CommandQueue;

impl CommandQueue {
    /// Create a new command channel with specified buffer size
    pub fn new(buffer_size: usize) -> (mpsc::Sender<MixerCommand>, mpsc::Receiver<MixerCommand>) {
        mpsc::channel(buffer_size)
    }
    
    /// Check if a command queue is full (for non-blocking operations)
    pub fn is_full(tx: &mpsc::Sender<MixerCommand>) -> bool {
        tx.capacity() == 0
    }
    
    /// Get the current queue length (approximate, for monitoring)
    pub fn get_queue_length(tx: &mpsc::Sender<MixerCommand>) -> usize {
        // Note: This is an approximation since MPSC doesn't provide exact length
        tx.capacity().saturating_sub(tx.capacity())
    }
}