// Audio stream lifecycle management
//
// This module handles the creation, management, and cleanup of audio input
// and output streams. It coordinates device switching, stream reconfiguration,
// and ensures proper resource cleanup.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::{info, warn, error};

use super::types::VirtualMixer;
use super::transformer::{AudioInputStream, AudioOutputStream};

impl VirtualMixer {
    /// Start the mixer and initialize audio processing
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            warn!("Mixer is already running");
            return Ok(());
        }

        info!("🚀 MIXER START: Starting virtual mixer...");
        
        // Reset timing metrics
        {
            let mut timing_metrics = self.timing_metrics.lock().await;
            timing_metrics.reset();
        }
        
        // Reset audio clock
        {
            let mut audio_clock = self.audio_clock.lock().await;
            audio_clock.reset();
        }
        
        self.is_running.store(true, Ordering::Relaxed);
        info!("✅ MIXER STARTED: Virtual mixer started successfully");
        
        Ok(())
    }

    /// Add an input stream for the specified device
    pub async fn add_input_stream(&self, device_id: &str) -> Result<()> {
        super::validation::validate_device_id(device_id)?;
        
        info!("🔌 INPUT STREAM: Adding input stream for device: {}", device_id);
        
        // Check if stream already exists
        {
            let input_streams = self.input_streams.lock().await;
            if input_streams.contains_key(device_id) {
                warn!("Input stream for device '{}' already exists", device_id);
                return Ok(());
            }
        }
        
        // Find the audio device
        let device_handle = self.audio_device_manager
            .find_audio_device(device_id, true)
            .await
            .with_context(|| format!("Failed to find input device '{}'", device_id))?;
        
        // Create input stream
        let input_stream = Arc::new(AudioInputStream::new(
            device_id.to_string(),
            device_id.to_string(), // Use device_id as name for now
            48000, // Default sample rate
        )?);
        
        // Initialize device health tracking
        if let Some(device_info) = self.audio_device_manager.get_device(device_id).await {
            let info = device_info;
            self.audio_device_manager.initialize_device_health(&info).await;
        }
        
        // Store the stream
        {
            let mut input_streams = self.input_streams.lock().await;
            input_streams.insert(device_id.to_string(), input_stream.clone());
        }
        
        info!("✅ INPUT STREAM: Successfully added input stream for device: {}", device_id);
        Ok(())
    }

    /// Set the output stream for the specified device
    pub async fn set_output_stream(&self, device_id: &str) -> Result<()> {
        super::validation::validate_device_id(device_id)?;
        
        info!("🔊 OUTPUT STREAM: Setting output stream for device: {}", device_id);
        
        // Find the audio device
        let device_handle = self.audio_device_manager
            .find_audio_device(device_id, false)
            .await
            .with_context(|| format!("Failed to find output device '{}'", device_id))?;
        
        // Create output stream
        let output_stream = Arc::new(AudioOutputStream::new(
            device_id.to_string(),
            device_id.to_string(), // Use device_id as name for now
            48000, // Default sample rate
        )?);
        
        // Initialize device health tracking
        if let Some(device_info) = self.audio_device_manager.get_device(device_id).await {
            let info = device_info;
            self.audio_device_manager.initialize_device_health(&info).await;
        }
        
        // Store the stream (replace existing)
        {
            let mut output_stream_guard = self.output_stream.lock().await;
            *output_stream_guard = Some(output_stream.clone());
        }
        
        // Also add to multiple outputs map
        {
            let mut output_streams = self.output_streams.lock().await;
            output_streams.insert(device_id.to_string(), output_stream);
        }
        
        // Track active device
        {
            let mut active_devices = self.active_output_devices.lock().await;
            active_devices.insert(device_id.to_string());
        }
        
        info!("✅ OUTPUT STREAM: Successfully set output stream for device: {}", device_id);
        Ok(())
    }

    /// Remove an input stream
    pub async fn remove_input_stream(&self, device_id: &str) -> Result<()> {
        super::validation::validate_device_id(device_id)?;
        
        info!("🔌 INPUT STREAM: Removing input stream for device: {}", device_id);
        
        // Remove from input streams
        let removed_stream = {
            let mut input_streams = self.input_streams.lock().await;
            input_streams.remove(device_id)
        };
        
        if removed_stream.is_some() {
            info!("✅ INPUT STREAM: Successfully removed input stream for device: {}", device_id);
        } else {
            warn!("Input stream for device '{}' not found", device_id);
        }
        
        Ok(())
    }

    /// Remove an output stream
    pub async fn remove_output_stream(&self, device_id: &str) -> Result<()> {
        super::validation::validate_device_id(device_id)?;
        
        info!("🔊 OUTPUT STREAM: Removing output stream for device: {}", device_id);
        
        // Remove from output streams
        {
            let mut output_streams = self.output_streams.lock().await;
            output_streams.remove(device_id);
        }
        
        // Remove from active devices
        {
            let mut active_devices = self.active_output_devices.lock().await;
            active_devices.remove(device_id);
        }
        
        // If this was the primary output, clear it
        {
            let mut primary_output = self.output_stream.lock().await;
            if let Some(ref stream) = *primary_output {
                if stream.get_device_id() == device_id {
                    *primary_output = None;
                }
            }
        }
        
        info!("✅ OUTPUT STREAM: Successfully removed output stream for device: {}", device_id);
        Ok(())
    }

    /// Stop the mixer and cleanup resources
    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            info!("Mixer is already stopped");
            return Ok(());
        }

        info!("🛑 MIXER STOP: Stopping virtual mixer...");
        
        self.is_running.store(false, Ordering::Relaxed);
        
        // Clear all input streams
        {
            let mut input_streams = self.input_streams.lock().await;
            input_streams.clear();
        }
        
        // Clear output streams
        {
            let mut output_stream = self.output_stream.lock().await;
            *output_stream = None;
        }
        
        {
            let mut output_streams = self.output_streams.lock().await;
            output_streams.clear();
        }
        
        // Clear active devices tracking
        {
            let mut active_devices = self.active_output_devices.lock().await;
            active_devices.clear();
        }
        
        #[cfg(target_os = "macos")]
        {
            let mut coreaudio_stream = self.coreaudio_stream.lock().await;
            *coreaudio_stream = None;
        }
        
        info!("✅ MIXER STOPPED: Virtual mixer stopped successfully");
        Ok(())
    }

    /// Get information about active streams
    pub async fn get_stream_info(&self) -> StreamInfo {
        let input_count = {
            let input_streams = self.input_streams.lock().await;
            input_streams.len()
        };
        
        let output_count = {
            let output_streams = self.output_streams.lock().await;
            output_streams.len()
        };
        
        let active_devices = {
            let active_devices = self.active_output_devices.lock().await;
            active_devices.clone()
        };
        
        let is_running = self.is_running.load(Ordering::Relaxed);
        
        StreamInfo {
            is_running,
            input_stream_count: input_count,
            output_stream_count: output_count,
            active_output_devices: active_devices.into_iter().collect(),
        }
    }

    /// Check if a specific device is currently active
    pub async fn is_device_active(&self, device_id: &str) -> bool {
        // Check input streams
        {
            let input_streams = self.input_streams.lock().await;
            if input_streams.contains_key(device_id) {
                return true;
            }
        }
        
        // Check output streams
        {
            let active_devices = self.active_output_devices.lock().await;
            if active_devices.contains(device_id) {
                return true;
            }
        }
        
        false
    }
}

/// Information about current stream state
#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub is_running: bool,
    pub input_stream_count: usize,
    pub output_stream_count: usize,
    pub active_output_devices: Vec<String>,
}

impl StreamInfo {
    /// Check if any streams are active
    pub fn has_active_streams(&self) -> bool {
        self.input_stream_count > 0 || self.output_stream_count > 0
    }
    
    /// Get total stream count
    pub fn total_stream_count(&self) -> usize {
        self.input_stream_count + self.output_stream_count
    }
}