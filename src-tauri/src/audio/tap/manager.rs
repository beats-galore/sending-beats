// Application audio manager - High-level orchestration and API
//
// This module provides the public API for application audio discovery and
// capture permissions. Capture itself is handled by ScreenCaptureKit.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};

use super::process_discovery::ApplicationDiscovery;
use super::types::{ProcessInfo, TapStats};

/// High-level manager for application audio discovery and capture permissions
///
/// Per-application capture itself runs through ScreenCaptureKit
/// (`audio::screencapture`). The CoreAudio process-tap implementation this
/// manager was originally built around was a dead end and has been removed, so
/// there are no taps to track here — the capture-lifecycle methods below report
/// empty state rather than pretending otherwise.
#[derive(Clone)]
pub struct ApplicationAudioManager {
    discovery: Arc<Mutex<ApplicationDiscovery>>,
    permission_granted: Arc<RwLock<bool>>,
}

impl ApplicationAudioManager {
    pub fn new() -> Self {
        Self {
            discovery: Arc::new(Mutex::new(ApplicationDiscovery::new())),
            permission_granted: Arc::new(RwLock::new(false)),
        }
    }

    /// Check and request audio capture permissions
    pub async fn request_permissions(&self) -> Result<bool> {
        info!("Requesting audio capture permissions...");

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

    /// Stop capturing audio from a specific application
    ///
    /// No-op: capture lifecycle is owned by the ScreenCaptureKit streams held in
    /// the mixer's stream manager, not by this type.
    pub async fn stop_capturing_app(&self, _pid: u32) -> Result<()> {
        Ok(())
    }

    /// Statistics for taps owned by this manager, of which there are none
    pub async fn get_tap_stats(&self) -> Vec<TapStats> {
        Vec::new()
    }

    /// Check if permissions are currently granted
    pub async fn has_permissions(&self) -> bool {
        *self.permission_granted.read().await
    }

    /// Captures owned by this manager, of which there are none
    pub async fn get_active_captures(&self) -> Vec<ProcessInfo> {
        Vec::new()
    }

    /// Stop all captures owned by this manager
    ///
    /// No-op: see `stop_capturing_app`.
    pub async fn stop_all_captures(&self) -> Result<()> {
        Ok(())
    }

    /// Clean up stale captures owned by this manager
    ///
    /// Always zero: there is no tap state to go stale.
    pub async fn cleanup_stale_taps(&self) -> Result<usize> {
        Ok(0)
    }

    /// Shutdown the manager and cleanup resources
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down ApplicationAudioManager");
        Ok(())
    }
}
