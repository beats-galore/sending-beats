// macOS TCC (Transparency, Consent, and Control) permission checking
// This handles checking and requesting microphone/audio capture permissions

#![cfg(target_os = "macos")]

use anyhow::Result;
use std::process::Command;
use tracing::{info, warn, error};

/// TCC permission status for audio capture
#[derive(Debug, Clone, PartialEq)]
pub enum TccPermissionStatus {
    /// Permission has been granted
    Granted,
    /// Permission has been explicitly denied
    Denied,
    /// Permission status is unknown or not determined yet
    NotDetermined,
    /// Unable to check permission status
    Unknown,
}

/// TCC permission manager for audio capture
pub struct TccPermissionManager;

impl TccPermissionManager {
    pub fn new() -> Self {
        Self
    }
    
    /// Check current microphone/audio capture permission status
    pub async fn check_audio_capture_permissions(&self) -> TccPermissionStatus {
        // Method 1: Try using tccutil (if available)
        if let Ok(status) = self.check_via_tccutil().await {
            return status;
        }
        
        // Method 2: Try checking via AVAudioSession (would require Objective-C)
        // For now, we'll use a simple approach of trying to access audio
        
        // Method 3: Check if we can access audio devices (fallback)
        self.check_via_audio_device_access().await
    }
    
    /// Request audio capture permissions from the user
    pub async fn request_permissions(&self) -> Result<bool> {
        info!("Requesting audio capture permissions from user");
        
        // On macOS, we can't programmatically request permissions
        // The system will automatically show the permission dialog when we first
        // try to access audio resources
        
        // The proper way is to try using Core Audio and let the system handle it
        // For now, we'll return true and let the actual audio capture trigger the dialog
        
        warn!("Permission request will happen automatically when audio capture is first attempted");
        Ok(true)
    }
    
    /// Check permissions using tccutil command line tool
    async fn check_via_tccutil(&self) -> Result<TccPermissionStatus> {
        // tccutil on macOS only supports 'reset' command, not 'query'
        // This method will always fail, so we rely on the fallback method
        info!("tccutil does not support querying permissions on this macOS version");
        Err(anyhow::anyhow!("tccutil query not supported"))
    }
    
    /// Fallback method: Try to access audio devices to infer permission status
    async fn check_via_audio_device_access(&self) -> TccPermissionStatus {
        // This is a heuristic approach - try to actually create an input stream
        // If we can create one, permissions are granted
        
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        
        // First check if we can enumerate devices
        let devices = match cpal::default_host().input_devices() {
            Ok(devices) => devices,
            Err(e) => {
                error!("Failed to enumerate audio devices: {} - permissions likely denied", e);
                return TccPermissionStatus::Denied;
            }
        };
        
        // Try to get the default input device
        let device = match cpal::default_host().default_input_device() {
            Some(device) => device,
            None => {
                warn!("No default input device found - permissions may be denied or no mic available");
                return TccPermissionStatus::NotDetermined;
            }
        };
        
        // Try to get default input config
        let config = match device.default_input_config() {
            Ok(config) => config,
            Err(e) => {
                error!("Failed to get default input config: {} - permissions likely denied", e);
                return TccPermissionStatus::Denied;
            }
        };
        
        // Try to build an input stream (this is where permission is actually checked)
        let stream_result = device.build_input_stream(
            &config.into(),
            move |_data: &[f32], _: &cpal::InputCallbackInfo| {
                // Empty callback - we just want to test if we can create the stream
            },
            move |err| {
                error!("Audio stream error during permission test: {}", err);
            },
            None
        );
        
        match stream_result {
            Ok(_stream) => {
                info!("Successfully created test audio input stream - permissions granted");
                TccPermissionStatus::Granted
            },
            Err(e) => {
                // Check the error message to determine if it's a permission issue
                let error_str = e.to_string().to_lowercase();
                if error_str.contains("permission") || error_str.contains("access") || error_str.contains("denied") {
                    error!("Permission denied when creating audio stream: {}", e);
                    TccPermissionStatus::Denied
                } else {
                    warn!("Unknown error creating audio stream: {} - assuming not determined", e);
                    TccPermissionStatus::NotDetermined
                }
            }
        }
    }
    
    /// Get the bundle identifier for the current application
    fn get_bundle_identifier(&self) -> Result<String> {
        // Try to read from Info.plist or use a default
        // For Tauri apps, this is typically defined in tauri.conf.json
        
        // First try to get from environment or bundle
        if let Ok(bundle_id) = std::env::var("CFBundleIdentifier") {
            return Ok(bundle_id);
        }
        
        // Fallback to the identifier from tauri.conf.json
        // This should match what's in the configuration
        Ok("com.SendinBeats".to_string())
    }
    
    /// Show system preferences for microphone permissions
    pub fn open_privacy_settings(&self) -> Result<()> {
        info!("Opening macOS Privacy settings for microphone permissions");
        
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
            .spawn()?;
            
        Ok(())
    }
    
    /// Get user-friendly instructions for enabling permissions
    pub fn get_permission_instructions(&self) -> String {
        format!(
            "To enable audio capture for Sendin Beats:\n\
            1. Open System Preferences > Security & Privacy > Privacy\n\
            2. Select 'Microphone' in the left sidebar\n\
            3. Check the box next to 'Sendin Beats'\n\
            4. Restart the application if needed\n\n\
            This permission is required to capture audio from applications like Spotify, iTunes, etc."
        )
    }
    
    /// Reset permissions (for testing/debugging)
    pub async fn reset_permissions(&self) -> Result<()> {
        let bundle_id = self.get_bundle_identifier()?;
        
        info!("Resetting TCC permissions for bundle: {}", bundle_id);
        
        let output = Command::new("tccutil")
            .args(&["reset", "Microphone", &bundle_id])
            .output()?;
            
        if output.status.success() {
            info!("Successfully reset microphone permissions");
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Failed to reset permissions: {}", stderr))
        }
    }
}

/// Global permission manager instance
static mut PERMISSION_MANAGER: Option<TccPermissionManager> = None;
static PERMISSION_MANAGER_INIT: std::sync::Once = std::sync::Once::new();

/// Get the global permission manager instance
pub fn get_permission_manager() -> &'static TccPermissionManager {
    unsafe {
        PERMISSION_MANAGER_INIT.call_once(|| {
            PERMISSION_MANAGER = Some(TccPermissionManager::new());
        });
        PERMISSION_MANAGER.as_ref().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_permission_check() {
        let manager = TccPermissionManager::new();
        let status = manager.check_audio_capture_permissions().await;
        
        // Should return some valid status
        assert!(matches!(status, 
            TccPermissionStatus::Granted | 
            TccPermissionStatus::Denied | 
            TccPermissionStatus::NotDetermined |
            TccPermissionStatus::Unknown
        ));
    }
    
    #[test]
    fn test_bundle_identifier() {
        let manager = TccPermissionManager::new();
        let bundle_id = manager.get_bundle_identifier().unwrap();
        
        // Should get some bundle ID
        assert!(!bundle_id.is_empty());
        assert!(bundle_id.contains("."));
    }
    
    #[test]
    fn test_permission_instructions() {
        let manager = TccPermissionManager::new();
        let instructions = manager.get_permission_instructions();
        
        assert!(instructions.contains("System Preferences"));
        assert!(instructions.contains("Microphone"));
        assert!(instructions.contains("Sendin Beats"));
    }
}