// Application audio manager - High-level orchestration and API
//
// This module provides the main public API for application audio capture,
// orchestrating between process discovery, tap creation, and mixer integration.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::{error, info, warn};

use super::mixer::stream_management::AudioInputStream;
use super::tap::process_discovery::ApplicationDiscovery;
use super::tap::types::{ApplicationAudioError, ProcessInfo, TapStats};
use super::tap::virtual_stream::get_virtual_input_registry;

#[cfg(target_os = "macos")]
use super::tap::core_audio_tap::ApplicationAudioTap;

/// High-level manager for application audio capture
#[derive(Clone)]
pub struct ApplicationAudioManager {
    discovery: Arc<Mutex<ApplicationDiscovery>>,
    #[cfg(target_os = "macos")]
    active_taps: Arc<RwLock<HashMap<u32, ApplicationAudioTap>>>, // PID -> Tap
    #[cfg(not(target_os = "macos"))]
    active_taps: Arc<RwLock<HashMap<u32, ()>>>,
    permission_granted: Arc<RwLock<bool>>,
    max_concurrent_captures: usize,
    cleanup_handle: Arc<StdMutex<Option<tokio::task::JoinHandle<()>>>>,
    should_stop_cleanup: Arc<std::sync::atomic::AtomicBool>,
}

impl ApplicationAudioManager {
    pub fn new() -> Self {
        Self {
            discovery: Arc::new(Mutex::new(ApplicationDiscovery::new())),
            active_taps: Arc::new(RwLock::new(HashMap::new())),
            permission_granted: Arc::new(RwLock::new(false)),
            max_concurrent_captures: 4, // Limit to prevent performance issues
            cleanup_handle: Arc::new(StdMutex::new(None)),
            should_stop_cleanup: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Ensure cleanup task is running (lazy startup)
    fn ensure_cleanup_task_started(&self) {
        if let Ok(cleanup_handle_guard) = self.cleanup_handle.try_lock() {
            if cleanup_handle_guard.is_none() {
                drop(cleanup_handle_guard);
                self.start_cleanup_task();
            }
        }
    }

    /// Check and request audio capture permissions
    pub async fn request_permissions(&self) -> Result<bool> {
        info!("Requesting audio capture permissions...");
        self.ensure_cleanup_task_started();

        #[cfg(target_os = "macos")]
        {
            use crate::permissions::{get_permission_manager, TccPermissionStatus};

            let permission_manager = get_permission_manager();

            // First check current permission status
            let status = permission_manager.check_audio_capture_permissions().await;
            info!("Current permission status: {:?}", status);

            let granted = match status {
                TccPermissionStatus::Granted => {
                    info!("Audio capture permissions already granted");
                    true
                }
                TccPermissionStatus::Denied => {
                    warn!("Audio capture permissions denied by user");
                    info!(
                        "Instructions for enabling permissions:\n{}",
                        permission_manager.get_permission_instructions()
                    );
                    false
                }
                TccPermissionStatus::NotDetermined => {
                    info!("Permissions not determined - will be requested on first audio access");
                    // Let the system handle the permission request when we try to access audio
                    match permission_manager.request_permissions().await {
                        Ok(result) => result,
                        Err(e) => {
                            error!("Failed to request permissions: {}", e);
                            false
                        }
                    }
                }
                TccPermissionStatus::Unknown => {
                    warn!("Unable to determine permission status - assuming not granted");
                    false
                }
            };

            *self.permission_granted.write().await = granted;

            if !granted {
                info!("To manually enable permissions, run: open 'x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone'");
            }

            Ok(granted)
        }

        #[cfg(not(target_os = "macos"))]
        {
            warn!("Permission checking not implemented on this platform");
            *self.permission_granted.write().await = false;
            Ok(false)
        }
    }

    /// Get list of available audio applications
    pub async fn get_available_applications(&self) -> Result<Vec<ProcessInfo>> {
        let mut discovery = self.discovery.lock().await;
        discovery.scan_audio_applications()
    }

    /// Start capturing audio from a specific application
    #[cfg(target_os = "macos")]
    pub async fn start_capturing_app(&self, pid: u32) -> Result<broadcast::Receiver<Vec<f32>>> {
        // Ensure cleanup task is running
        self.ensure_cleanup_task_started();

        // Check permissions (actively check system, don't use cached value)
        if !self.check_audio_capture_permissions().await {
            return Err(ApplicationAudioError::PermissionDenied.into());
        }

        // Check concurrent capture limit
        let active_count = self.active_taps.read().await.len();
        if active_count >= self.max_concurrent_captures {
            return Err(ApplicationAudioError::TooManyCaptures {
                max: self.max_concurrent_captures,
            }
            .into());
        }

        // Get process info
        let discovery = self.discovery.lock().await;
        let process_info = discovery
            .get_process_info(pid)
            .ok_or_else(|| ApplicationAudioError::ApplicationNotFound { pid })?;
        drop(discovery);

        // Create and configure tap
        let mut tap = ApplicationAudioTap::new(process_info);

        // Attempt to create the tap
        tap.create_tap().await?;

        // Start capturing
        let receiver = tap
            .get_audio_sender()
            .ok_or(ApplicationAudioError::TapNotInitialized)?
            .subscribe();

        // Store the tap
        self.active_taps.write().await.insert(pid, tap);

        info!(
            "Started capturing audio from PID {} with lifecycle management",
            pid
        );
        Ok(receiver)
    }

    #[cfg(not(target_os = "macos"))]
    pub async fn start_capturing_app(&self, _pid: u32) -> Result<broadcast::Receiver<Vec<f32>>> {
        Err(ApplicationAudioError::UnsupportedSystem.into())
    }

    /// Create a virtual mixer input channel for an application's audio
    /// This integrates application audio capture with the existing mixer system
    pub async fn create_mixer_input_for_app(&self, pid: u32) -> Result<String> {
        info!("🎛️ Creating mixer input for application PID: {}", pid);

        // Get process info for naming FIRST
        let discovery = self.discovery.lock().await;
        let process_info = discovery
            .get_process_info(pid)
            .ok_or_else(|| ApplicationAudioError::ApplicationNotFound { pid })?;
        drop(discovery);

        let channel_name = format!("App: {}", process_info.name);
        let virtual_device_id = format!("app-{}", pid);

        // CRITICAL: Register virtual stream FIRST, before starting capture
        info!(
            "📡 Pre-registering virtual stream {} before capture",
            virtual_device_id
        );
        let bridge_buffer = Arc::new(tokio::sync::Mutex::new(Vec::<f32>::new()));
        self.register_virtual_input_stream_sync(
            virtual_device_id.clone(),
            channel_name.clone(),
            bridge_buffer.clone(),
        )
        .await?;

        // NOW start capturing from the application
        let audio_receiver = self.start_capturing_app(pid).await?;

        // Bridge the audio to the pre-registered stream
        self.bridge_tap_audio_to_existing_stream(pid, audio_receiver, bridge_buffer)
            .await?;

        info!(
            "✅ Created virtual mixer input '{}' for PID {} with pre-registered stream",
            channel_name, pid
        );
        Ok(channel_name)
    }

    /// Stop capturing audio from a specific application
    pub async fn stop_capturing_app(&self, pid: u32) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            if let Some(mut tap) = self.active_taps.write().await.remove(&pid) {
                tap.cleanup().await?;
                info!("Stopped capturing audio from PID {}", pid);
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            self.active_taps.write().await.remove(&pid);
        }

        Ok(())
    }

    /// Get statistics for all active taps
    pub async fn get_tap_stats(&self) -> Vec<TapStats> {
        let taps = self.active_taps.read().await;
        let mut stats = Vec::new();

        for tap in taps.values() {
            stats.push(tap.get_stats().await);
        }

        stats.sort_by_key(|s| s.pid);
        stats
    }

    /// Check if audio capture permissions are granted
    async fn check_audio_capture_permissions(&self) -> bool {
        *self.permission_granted.read().await
    }

    /// Register virtual stream synchronously BEFORE capture starts  
    async fn register_virtual_input_stream_sync(
        &self,
        virtual_device_id: String,
        channel_name: String,
        bridge_buffer: Arc<tokio::sync::Mutex<Vec<f32>>>,
    ) -> Result<()> {
        info!(
            "📡 SYNC: Registering virtual input stream: {} ({})",
            channel_name, virtual_device_id
        );

        // Create the AudioInputStream immediately and register it
        let audio_input_stream =
            Arc::new(crate::audio::mixer::stream_management::AudioInputStream::new(
                virtual_device_id.clone(),
                channel_name.clone(),
                48000,
            )?);

        // Store in global registry IMMEDIATELY
        self.add_to_global_mixer_sync(virtual_device_id.clone(), audio_input_stream)
            .await?;

        info!(
            "✅ SYNC: Virtual stream {} registered and ready for mixer",
            virtual_device_id
        );
        Ok(())
    }

    /// Synchronously add virtual stream to global mixer registry
    async fn add_to_global_mixer_sync(
        &self,
        device_id: String,
        audio_input_stream: Arc<crate::audio::mixer::stream_management::AudioInputStream>,
    ) -> Result<()> {
        info!(
            "🔗 SYNC: Adding virtual stream {} to global mixer registry",
            device_id
        );

        // Use centralized registry function
        let registry = get_virtual_input_registry();
        if let Ok(mut reg) = registry.lock() {
            reg.insert(device_id.clone(), audio_input_stream);
            info!(
                "✅ SYNC: Registered virtual stream {} in global registry (total: {})",
                device_id,
                reg.len()
            );
        } else {
            return Err(anyhow::anyhow!("Failed to lock virtual input registry"));
        }

        Ok(())
    }

    /// Bridge tap audio data to an existing registered stream
    async fn bridge_tap_audio_to_existing_stream(
        &self,
        pid: u32,
        mut audio_receiver: broadcast::Receiver<Vec<f32>>,
        bridge_buffer: Arc<tokio::sync::Mutex<Vec<f32>>>,
    ) -> Result<()> {
        let virtual_device_id = format!("app-{}", pid);
        info!(
            "🌉 Setting up audio bridge for existing stream {}",
            virtual_device_id
        );

        let bridge_buffer_for_task = bridge_buffer.clone();
        let virtual_device_id_for_task = virtual_device_id.clone();

        tokio::spawn(async move {
            info!(
                "🔗 Audio bridge task started for {}",
                virtual_device_id_for_task
            );
            let mut sample_count = 0u64;

            while let Ok(audio_samples) = audio_receiver.recv().await {
                sample_count += audio_samples.len() as u64;

                // Calculate levels for monitoring
                let peak_level = audio_samples
                    .iter()
                    .map(|&s| s.abs())
                    .fold(0.0f32, f32::max);
                let rms_level = (audio_samples.iter().map(|&s| s * s).sum::<f32>()
                    / audio_samples.len() as f32)
                    .sqrt();

                // Store samples in the bridge buffer (same pattern as CPAL input streams)
                if let Ok(mut buffer) = bridge_buffer_for_task.try_lock() {
                    buffer.extend_from_slice(&audio_samples);

                    // Prevent buffer overflow - same logic as regular input streams
                    let max_buffer_size = 48000; // 1 second at 48kHz
                    if buffer.len() > max_buffer_size * 2 {
                        let keep_size = max_buffer_size;
                        let buffer_len = buffer.len();
                        let new_buffer = buffer.split_off(buffer_len - keep_size);
                        *buffer = new_buffer;
                    }

                    // Log periodically
                    if sample_count % 4800 == 0 || (peak_level > 0.01 && sample_count % 1000 == 0) {
                        info!("🌉 BRIDGE [{}]: {} samples bridged to mixer, peak: {:.4}, rms: {:.4}, buffer: {} samples", 
                            virtual_device_id_for_task, audio_samples.len(), peak_level, rms_level, buffer.len());
                    }
                } else {
                    warn!(
                        "Failed to lock bridge buffer for {}",
                        virtual_device_id_for_task
                    );
                }
            }

            info!(
                "🔗 Audio bridge task ended for {}",
                virtual_device_id_for_task
            );
        });

        Ok(())
    }

    /// Start cleanup task for managing tap lifecycle
    fn start_cleanup_task(&self) {
        let active_taps = self.active_taps.clone();
        let should_stop = self.should_stop_cleanup.clone();
        let cleanup_handle = self.cleanup_handle.clone();

        let handle = tokio::spawn(async move {
            info!("🧹 Application audio cleanup task started");

            while !should_stop.load(std::sync::atomic::Ordering::Relaxed) {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;

                // Check for dead processes and clean up their taps
                #[cfg(target_os = "macos")]
                {
                    let mut taps_to_remove = Vec::new();
                    let taps = active_taps.read().await;

                    for (pid, tap) in taps.iter() {
                        let stats = tap.get_stats().await;
                        if !stats.process_alive || stats.error_count > 5 {
                            info!(
                                "🧹 Marking tap for cleanup: PID {} (alive: {}, errors: {})",
                                pid, stats.process_alive, stats.error_count
                            );
                            taps_to_remove.push(*pid);
                        }
                    }

                    drop(taps);

                    // Clean up dead taps
                    if !taps_to_remove.is_empty() {
                        let mut taps = active_taps.write().await;
                        for pid in taps_to_remove {
                            if let Some(mut tap) = taps.remove(&pid) {
                                if let Err(e) = tap.cleanup().await {
                                    warn!("Failed to cleanup tap for PID {}: {}", pid, e);
                                }
                                info!("🧹 Cleaned up tap for PID {}", pid);
                            }
                        }
                    }
                }
            }

            info!("🧹 Application audio cleanup task stopped");
        });

        if let Ok(mut cleanup_guard) = cleanup_handle.try_lock() {
            *cleanup_guard = Some(handle);
        };
    }

    /// Get a virtual input stream from the ApplicationAudioManager registry
    pub async fn get_virtual_input_stream(&self, device_id: &str) -> Option<Arc<AudioInputStream>> {
        info!(
            "🔍 Looking up virtual input stream for device: {}",
            device_id
        );

        let virtual_streams = crate::audio::ApplicationAudioManager::get_virtual_input_streams();
        if let Some(stream) = virtual_streams.get(device_id) {
            info!("✅ Found virtual input stream for device: {}", device_id);
            Some(stream.clone())
        } else {
            info!(
                "❌ No virtual input stream found for device: {} (available: {:?})",
                device_id,
                virtual_streams.keys().collect::<Vec<_>>()
            );
            None
        }
    }

    /// Get all registered virtual input streams (for mixer integration)
    pub fn get_virtual_input_streams(
    ) -> HashMap<String, Arc<crate::audio::mixer::stream_management::AudioInputStream>> {
        // Use centralized registry function
        let registry = get_virtual_input_registry();
        if let Ok(reg) = registry.lock() {
            reg.clone()
        } else {
            HashMap::new()
        }
    }

    /// Check if permissions are currently granted
    pub async fn has_permissions(&self) -> bool {
        *self.permission_granted.read().await
    }

    /// Get list of active captures with process info
    pub async fn get_active_captures(&self) -> Vec<ProcessInfo> {
        let pids: Vec<u32> = self.active_taps.read().await.keys().copied().collect();
        let mut result = Vec::new();

        // Get process info for each active capture
        if let Ok(available_apps) = self.get_available_applications().await {
            for pid in pids {
                if let Some(process_info) = available_apps.iter().find(|app| app.pid == pid) {
                    result.push(process_info.clone());
                }
            }
        }

        result
    }

    /// Stop all active captures
    pub async fn stop_all_captures(&self) -> Result<()> {
        let pids: Vec<u32> = self.active_taps.read().await.keys().copied().collect();

        for pid in pids {
            if let Err(e) = self.stop_capturing_app(pid).await {
                warn!("Failed to stop capture for PID {}: {}", pid, e);
            }
        }

        Ok(())
    }

    /// Clean up stale/dead taps
    pub async fn cleanup_stale_taps(&self) -> Result<usize> {
        #[cfg(target_os = "macos")]
        {
            let mut taps_to_remove = Vec::new();
            let taps = self.active_taps.read().await;

            for (pid, tap) in taps.iter() {
                let stats = tap.get_stats().await;
                if !stats.process_alive || stats.error_count > 5 {
                    taps_to_remove.push(*pid);
                }
            }

            drop(taps);

            let removed_count = taps_to_remove.len();

            for pid in taps_to_remove {
                if let Err(e) = self.stop_capturing_app(pid).await {
                    warn!("Failed to cleanup tap for PID {}: {}", pid, e);
                }
            }

            return Ok(removed_count);
        }

        #[cfg(not(target_os = "macos"))]
        Ok(0)
    }

    /// Shutdown the manager and cleanup resources
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down ApplicationAudioManager");

        // Stop cleanup task
        self.should_stop_cleanup
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // Stop all captures
        self.stop_all_captures().await?;

        info!("ApplicationAudioManager shutdown complete");
        Ok(())
    }
}

impl Drop for ApplicationAudioManager {
    fn drop(&mut self) {
        // Signal cleanup task to stop
        self.should_stop_cleanup
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
