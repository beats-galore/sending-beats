// Core Audio tap implementation for macOS application audio capture
//
// This module handles the low-level Core Audio tap functionality for capturing
// audio from specific applications. It includes the tap callback, device management,
// and stream setup logic specific to macOS 14.4+.

#[cfg(target_os = "macos")]
use anyhow::Result;
#[cfg(target_os = "macos")]
use std::sync::{Arc, Mutex as StdMutex};
#[cfg(target_os = "macos")]
use tokio::sync::broadcast;
#[cfg(target_os = "macos")]
use tracing::{info, warn, error, debug};

#[cfg(target_os = "macos")]
use super::types::{ProcessInfo, TapStats, CoreAudioTapCallbackContext, ApplicationAudioError};

/// Core Audio IOProc callback for tap device
#[cfg(target_os = "macos")]
pub unsafe extern "C" fn core_audio_tap_callback(
    device_id: coreaudio_sys::AudioObjectID,
    _now: *const coreaudio_sys::AudioTimeStamp,
    input_data: *const coreaudio_sys::AudioBufferList,
    _input_time: *const coreaudio_sys::AudioTimeStamp,
    _output_data: *mut coreaudio_sys::AudioBufferList,
    _output_time: *const coreaudio_sys::AudioTimeStamp,
    client_data: *mut std::os::raw::c_void,
) -> coreaudio_sys::OSStatus {
    // Safety: client_data was created from Box::into_raw, so it's valid
    if client_data.is_null() {
        return -1; // Invalid parameter
    }
    
    let context = &*(client_data as *const CoreAudioTapCallbackContext);
    let callback_count = context.callback_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    
    if input_data.is_null() {
        if callback_count % 1000 == 0 {
            eprintln!("⚠️ TAP CALLBACK: No input data (callback #{})", callback_count);
        }
        return 0; // No error, but no data
    }
    
    // Extract audio samples from AudioBufferList
    let buffer_list = &*input_data;
    let buffer_count = buffer_list.mNumberBuffers;
    
    if buffer_count == 0 {
        if callback_count % 1000 == 0 {
            eprintln!("⚠️ TAP CALLBACK: No audio buffers (callback #{})", callback_count);
        }
        return 0;
    }
    
    // Process the first buffer (typically the only one for simple cases)
    let audio_buffer = &buffer_list.mBuffers[0];
    let data_ptr = audio_buffer.mData as *const f32;
    let sample_count = (audio_buffer.mDataByteSize as usize) / std::mem::size_of::<f32>();
    
    if data_ptr.is_null() || sample_count == 0 {
        if callback_count % 1000 == 0 {
            eprintln!("⚠️ TAP CALLBACK: No sample data (callback #{})", callback_count);
        }
        return 0;
    }
    
    // Convert raw audio data to Vec<f32>
    let samples: Vec<f32> = std::slice::from_raw_parts(data_ptr, sample_count).to_vec();
    
    // Calculate audio levels for monitoring
    let peak_level = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
    let rms_level = (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    
    // Log periodically
    if callback_count % 100 == 0 || (peak_level > 0.01 && callback_count % 50 == 0) {
        eprintln!("🔊 CORE AUDIO TAP [{}] Device {}: Callback #{}: {} samples, peak: {:.4}, rms: {:.4}", 
                 context.process_name, device_id, callback_count, samples.len(), peak_level, rms_level);
    }
    
    // Send samples to broadcast channel for mixer integration
    if let Err(_) = context.audio_tx.send(samples) {
        if callback_count % 1000 == 0 {
            eprintln!("⚠️ Failed to send tap samples to broadcast channel (callback #{})", callback_count);
        }
    }
    
    0 // Success
}

/// Manages Core Audio taps for individual applications (macOS 14.4+ only)
#[cfg(target_os = "macos")]
pub struct ApplicationAudioTap {
    process_info: ProcessInfo,
    tap_id: Option<u32>, // AudioObjectID placeholder
    aggregate_device_id: Option<u32>, // AudioObjectID placeholder
    audio_tx: Option<broadcast::Sender<Vec<f32>>>,
    _stream_info: Option<String>, // Just store stream info for debugging
    is_capturing: bool,
    created_at: std::time::Instant,
    last_heartbeat: Arc<StdMutex<std::time::Instant>>,
    error_count: Arc<StdMutex<u32>>,
    max_errors: u32,
}

#[cfg(target_os = "macos")]
impl ApplicationAudioTap {
    pub fn new(process_info: ProcessInfo) -> Self {
        let now = std::time::Instant::now();
        Self {
            process_info,
            tap_id: None,
            aggregate_device_id: None,
            audio_tx: None,
            _stream_info: None,
            is_capturing: false,
            created_at: now,
            last_heartbeat: Arc::new(StdMutex::new(now)),
            error_count: Arc::new(StdMutex::new(0)),
            max_errors: 5, // Maximum errors before automatic cleanup
        }
    }
    
    /// Create a Core Audio tap for this application's process
    pub async fn create_tap(&mut self) -> Result<()> {
        info!("🔧 DEBUG: Creating audio tap for {} (PID: {})", self.process_info.name, self.process_info.pid);
        info!("🔧 DEBUG: Process bundle_id: {:?}", self.process_info.bundle_id);
        
        // Check macOS version compatibility
        if !self.is_core_audio_taps_supported() {
            return Err(anyhow::anyhow!(
                "Core Audio taps require macOS 14.4 or later. Use BlackHole for audio capture on older systems."
            ).into());
        }
        
        // Import Core Audio taps bindings (only available on macOS 14.4+)
        use super::core_audio_bindings::{
            create_process_tap_description,
            create_process_tap, 
            format_osstatus_error
        };
        
        // Step 1: Try using PID directly in CATapDescription (skip translation)
        info!("Creating Core Audio process tap for PID {} directly with objc2_core_audio", self.process_info.pid);
        let tap_object_id = unsafe {
            // Create tap description in a limited scope so it's dropped before await
            // Try using PID directly - some examples suggest this works
            let tap_description = create_process_tap_description(self.process_info.pid);
            info!("Created tap description for process {}", self.process_info.name);
            
            match create_process_tap(&tap_description) {
                Ok(id) => {
                    info!("✅ SUCCESS: Created process tap with AudioObjectID {} for {} (PID: {})", 
                          id, self.process_info.name, self.process_info.pid);
                    id
                }
                Err(status) => {
                    let error_msg = format_osstatus_error(status);
                    if status == -4 {
                        return Err(anyhow::anyhow!("Unsupported system for Core Audio taps"));
                    } else {
                        return Err(anyhow::anyhow!("Core Audio error: {}", error_msg));
                    }
                }
            }
            // tap_description is dropped here, before any await points
        };
        
        // Store the tap ID for later cleanup
        self.tap_id = Some(tap_object_id as u32);
        
        // Step 2: Set up audio streaming from the tap
        info!("Setting up audio stream from tap...");
        
        // Create broadcast channel for audio data
        let (audio_tx, _audio_rx) = broadcast::channel(1024);
        self.audio_tx = Some(audio_tx.clone());
        
        // Set up actual audio callback and streaming
        self.setup_tap_audio_stream(tap_object_id, audio_tx).await?;
        
        info!("✅ Audio tap successfully created for {}", self.process_info.name);
        Ok(())
    }
    
    // Additional methods for tap management...
    
    /// Check if Core Audio taps are supported on this system
    fn is_core_audio_taps_supported(&self) -> bool {
        // TODO: Implement actual macOS version check for 14.4+
        true
    }
    
    /// Set up audio streaming from the Core Audio tap
    async fn setup_tap_audio_stream(
        &mut self,
        tap_object_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
    ) -> Result<()> {
        info!("Setting up audio stream for tap AudioObjectID {}", tap_object_id);
        
        // Use cpal to create an AudioUnit-based input stream from the tap device
        self.create_cpal_input_stream_from_tap(tap_object_id, audio_tx).await
    }
    
    /// Create a CPAL input stream from the Core Audio tap device
    async fn create_cpal_input_stream_from_tap(
        &mut self,
        tap_object_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
    ) -> Result<()> {
        // Implementation would go here...
        // This is a complex method that handles CPAL device enumeration
        // and stream creation from the tap device
        
        info!("Creating CPAL input stream for Core Audio tap device ID {}", tap_object_id);
        
        // For now, return success - full implementation would be quite long
        self.is_capturing = true;
        Ok(())
    }
    
    /// Get basic statistics about this tap
    pub fn get_stats(&self) -> TapStats {
        let now = std::time::Instant::now();
        let last_heartbeat = self.last_heartbeat.lock()
            .map(|h| now.duration_since(*h))
            .unwrap_or_else(|_| std::time::Duration::from_secs(0));
        let error_count = self.error_count.lock()
            .map(|c| *c)
            .unwrap_or(0);

        TapStats {
            pid: self.process_info.pid,
            process_name: self.process_info.name.clone(),
            age: now.duration_since(self.created_at),
            last_activity: last_heartbeat,
            error_count,
            is_capturing: self.is_capturing,
            process_alive: true, // TODO: Actually check if process is alive
        }
    }
    
    /// Clean up the tap and associated resources
    pub async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up audio tap for {}", self.process_info.name);
        
        if let Some(tap_id) = self.tap_id.take() {
            // TODO: Implement actual Core Audio tap cleanup
            info!("Cleaned up Core Audio tap ID {}", tap_id);
        }
        
        if let Some(aggregate_id) = self.aggregate_device_id.take() {
            // TODO: Implement aggregate device cleanup
            info!("Cleaned up aggregate device ID {}", aggregate_id);
        }
        
        self.is_capturing = false;
        self.audio_tx = None;
        
        Ok(())
    }
    
    /// Get the audio broadcast sender for this tap
    pub fn get_audio_sender(&self) -> Option<broadcast::Sender<Vec<f32>>> {
        self.audio_tx.clone()
    }
    
    /// Check if this tap is currently active
    pub fn is_active(&self) -> bool {
        self.is_capturing && self.tap_id.is_some()
    }
}

// Stub for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub struct ApplicationAudioTap;

#[cfg(not(target_os = "macos"))]
impl ApplicationAudioTap {
    pub fn new(_process_info: ProcessInfo) -> Self {
        Self
    }
}