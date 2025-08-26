use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};
use sysinfo::{System, Pid, Process};
use tracing::{info, warn, error, debug};

/// Information about a discovered process that might have audio capabilities
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub bundle_id: Option<String>,
    pub icon_path: Option<PathBuf>,
    pub is_audio_capable: bool,
    pub is_playing_audio: bool,
}

/// Discovers and tracks audio-capable applications on the system
pub struct ApplicationDiscovery {
    system: System,
    known_audio_apps: HashMap<String, String>, // process name -> bundle ID
    last_scan: std::time::Instant,
    scan_interval: std::time::Duration,
}

impl ApplicationDiscovery {
    pub fn new() -> Self {
        let mut known_audio_apps = HashMap::new();
        
        // Add well-known audio applications
        known_audio_apps.insert("Spotify".to_string(), "com.spotify.client".to_string());
        known_audio_apps.insert("iTunes".to_string(), "com.apple.iTunes".to_string());
        known_audio_apps.insert("Music".to_string(), "com.apple.Music".to_string());
        known_audio_apps.insert("Tidal".to_string(), "com.tidal.desktop".to_string());
        known_audio_apps.insert("YouTube Music Desktop".to_string(), "com.ytmusic.ytmusic".to_string());
        known_audio_apps.insert("Pandora".to_string(), "com.pandora.desktop".to_string());
        known_audio_apps.insert("SoundCloud".to_string(), "com.soundcloud.desktop".to_string());
        known_audio_apps.insert("Apple Music".to_string(), "com.apple.Music".to_string());
        known_audio_apps.insert("Amazon Music".to_string(), "com.amazon.music".to_string());
        known_audio_apps.insert("Deezer".to_string(), "com.deezer.desktop".to_string());
        known_audio_apps.insert("VLC".to_string(), "org.videolan.vlc".to_string());
        known_audio_apps.insert("IINA".to_string(), "com.colliderli.iina".to_string());
        known_audio_apps.insert("QuickTime Player".to_string(), "com.apple.QuickTimePlayerX".to_string());
        
        Self {
            system: System::new_all(),
            known_audio_apps,
            last_scan: std::time::Instant::now() - std::time::Duration::from_secs(10), // Force initial scan
            scan_interval: std::time::Duration::from_secs(5), // Scan every 5 seconds
        }
    }
    
    /// Scan for all audio-capable applications currently running
    pub fn scan_audio_applications(&mut self) -> Result<Vec<ProcessInfo>> {
        // Only scan if enough time has passed
        if self.last_scan.elapsed() < self.scan_interval {
            return self.get_cached_audio_applications();
        }
        
        debug!("Scanning for audio applications...");
        self.system.refresh_all();
        self.last_scan = std::time::Instant::now();
        
        let mut audio_processes = Vec::new();
        
        // Iterate through all running processes
        for (pid, process) in self.system.processes() {
            let process_name = process.name();
            
            // Check if this is a known audio application
            if let Some(bundle_id) = self.known_audio_apps.get(process_name) {
                let process_info = ProcessInfo {
                    pid: pid.as_u32(),
                    name: process_name.to_string(),
                    bundle_id: Some(bundle_id.clone()),
                    icon_path: self.get_app_icon_path(bundle_id),
                    is_audio_capable: true,
                    is_playing_audio: self.is_app_playing_audio(pid.as_u32()),
                };
                
                debug!("Found known audio app: {} (PID: {})", process_name, pid);
                audio_processes.push(process_info);
            }
            // Also check for processes that might be audio-capable based on name patterns
            else if self.might_be_audio_app(process_name) {
                let process_info = ProcessInfo {
                    pid: pid.as_u32(),
                    name: process_name.to_string(),
                    bundle_id: None,
                    icon_path: None,
                    is_audio_capable: true,
                    is_playing_audio: self.is_app_playing_audio(pid.as_u32()),
                };
                
                debug!("Found potential audio app: {} (PID: {})", process_name, pid);
                audio_processes.push(process_info);
            }
        }
        
        info!("Found {} audio-capable applications", audio_processes.len());
        Ok(audio_processes)
    }
    
    /// Get only the well-known audio applications
    pub fn get_known_audio_apps(&mut self) -> Result<Vec<ProcessInfo>> {
        let all_audio_apps = self.scan_audio_applications()?;
        Ok(all_audio_apps.into_iter()
            .filter(|app| app.bundle_id.is_some())
            .collect())
    }
    
    /// Check if an application might be audio-capable based on name patterns
    fn might_be_audio_app(&self, process_name: &str) -> bool {
        let audio_keywords = [
            "music", "audio", "sound", "player", "radio", "podcast", 
            "stream", "media", "video", "youtube", "netflix", "hulu"
        ];
        
        let name_lower = process_name.to_lowercase();
        audio_keywords.iter().any(|keyword| name_lower.contains(keyword))
    }
    
    /// Check if an application is currently playing audio (placeholder implementation)
    fn is_app_playing_audio(&self, _pid: u32) -> bool {
        // TODO: Implement actual audio playback detection
        // This would require Core Audio APIs to check if a process is producing audio
        // For now, we'll assume any running audio app might be playing audio
        false
    }
    
    /// Get the icon path for an application bundle (placeholder implementation)
    fn get_app_icon_path(&self, _bundle_id: &str) -> Option<PathBuf> {
        // TODO: Implement app icon discovery
        // This would involve querying the app bundle for its icon file
        None
    }
    
    /// Get cached audio applications if scan hasn't expired
    fn get_cached_audio_applications(&self) -> Result<Vec<ProcessInfo>> {
        // TODO: Implement caching mechanism
        // For now, return empty vec to force a scan
        Ok(Vec::new())
    }
    
    /// Refresh the system process list
    pub fn refresh(&mut self) {
        self.system.refresh_all();
    }
    
    /// Get process info by PID
    pub fn get_process_info(&self, pid: u32) -> Option<ProcessInfo> {
        if let Some(process) = self.system.process(Pid::from_u32(pid)) {
            let process_name = process.name();
            let bundle_id = self.known_audio_apps.get(process_name).cloned();
            
            Some(ProcessInfo {
                pid,
                name: process_name.to_string(),
                bundle_id: bundle_id.clone(),
                icon_path: bundle_id.as_ref().and_then(|bid| self.get_app_icon_path(bid)),
                is_audio_capable: bundle_id.is_some() || self.might_be_audio_app(process_name),
                is_playing_audio: self.is_app_playing_audio(pid),
            })
        } else {
            None
        }
    }
}

/// Manages Core Audio taps for individual applications (macOS 14.4+ only)
#[cfg(target_os = "macos")]
pub struct ApplicationAudioTap {
    process_info: ProcessInfo,
    tap_id: Option<u32>, // AudioObjectID placeholder
    aggregate_device_id: Option<u32>, // AudioObjectID placeholder
    audio_tx: Option<broadcast::Sender<Vec<f32>>>,
    is_capturing: bool,
}

#[cfg(target_os = "macos")]
impl ApplicationAudioTap {
    pub fn new(process_info: ProcessInfo) -> Self {
        Self {
            process_info,
            tap_id: None,
            aggregate_device_id: None,
            audio_tx: None,
            is_capturing: false,
        }
    }
    
    /// Create a Core Audio tap for this application's process
    pub async fn create_tap(&mut self) -> Result<()> {
        info!("Creating audio tap for {} (PID: {})", self.process_info.name, self.process_info.pid);
        
        // Check macOS version compatibility
        if !self.is_core_audio_taps_supported() {
            return Err(anyhow::anyhow!(
                "Core Audio taps require macOS 14.4 or later. Use BlackHole for audio capture on older systems."
            ));
        }
        
        // Import Core Audio taps bindings (only available on macOS)
        #[cfg(target_os = "macos")]
        {
            use crate::coreaudio_taps::{
                CATapDescription, 
                create_process_tap, 
                translate_pid_to_audio_object,
                format_osstatus_error
            };
            
            // Step 1: Translate PID to AudioObjectID
            info!("Translating PID {} to AudioObjectID", self.process_info.pid);
            let process_audio_object_id = unsafe {
                match translate_pid_to_audio_object(self.process_info.pid) {
                    Ok(id) => {
                        info!("Successfully translated PID {} to AudioObjectID {}", self.process_info.pid, id);
                        id
                    }
                    Err(status) => {
                        let error_msg = format_osstatus_error(status);
                        return Err(anyhow::anyhow!(
                            "Failed to translate PID {} to AudioObjectID: {} ({})", 
                            self.process_info.pid, error_msg, status
                        ));
                    }
                }
            };
            
            // Step 2: Create tap description for this process
            let tap_description = CATapDescription::new_for_process(process_audio_object_id);
            info!("Created tap description for process {}: {:?}", self.process_info.name, tap_description);
            
            // Step 3: Create the Core Audio process tap
            info!("Creating Core Audio process tap...");
            let tap_object_id = unsafe {
                match create_process_tap(&tap_description) {
                    Ok(id) => {
                        info!("Successfully created process tap with AudioObjectID {}", id);
                        id
                    }
                    Err(status) => {
                        let error_msg = format_osstatus_error(status);
                        return Err(anyhow::anyhow!(
                            "Failed to create process tap for {}: {} ({})", 
                            self.process_info.name, error_msg, status
                        ));
                    }
                }
            };
            
            // Store the tap ID for later cleanup
            self.tap_id = Some(tap_object_id as u32);
            
            // Step 4: TODO - Create aggregate device with the tap
            // This would involve:
            // - Creating an AudioAggregateDeviceDescription
            // - Calling AudioHardwareCreateAggregateDevice 
            // - Setting up the audio format and callback
            // - Starting the audio device IO
            
            info!("⚠️ Process tap created but aggregate device setup not yet implemented");
            info!("Next steps: Create aggregate device and set up audio callback");
            
            // Create broadcast channel for audio samples
            let (tx, _rx) = broadcast::channel(1024);
            self.audio_tx = Some(tx);
            
            info!("✅ Audio tap successfully created for {}", self.process_info.name);
            Ok(())
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Err(anyhow::anyhow!("Application audio capture is only supported on macOS"))
        }
    }
    
    /// Start capturing audio from the tapped application
    pub fn start_capture(&mut self) -> Result<broadcast::Receiver<Vec<f32>>> {
        if self.audio_tx.is_none() {
            return Err(anyhow::anyhow!("Audio tap not created. Call create_tap() first."));
        }
        
        info!("Starting audio capture for {}", self.process_info.name);
        
        // TODO: Implement actual audio capture start
        // This involves starting the audio device IO
        
        self.is_capturing = true;
        
        // Return a receiver for the audio samples
        Ok(self.audio_tx.as_ref().unwrap().subscribe())
    }
    
    /// Stop capturing audio
    pub fn stop_capture(&mut self) -> Result<()> {
        if self.is_capturing {
            info!("Stopping audio capture for {}", self.process_info.name);
            
            // TODO: Implement actual audio capture stop
            // This involves stopping the audio device IO
            
            self.is_capturing = false;
        }
        
        Ok(())
    }
    
    /// Check if Core Audio taps are supported on this system
    fn is_core_audio_taps_supported(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            
            // Get macOS version using sw_vers command
            if let Ok(output) = Command::new("sw_vers")
                .arg("-productVersion")
                .output()
            {
                if let Ok(version_str) = String::from_utf8(output.stdout) {
                    let version = version_str.trim();
                    if let Ok(parsed_version) = self.parse_macos_version(version) {
                        // Core Audio taps require macOS 14.4+
                        return parsed_version >= (14, 4, 0);
                    }
                }
            }
            
            warn!("Could not determine macOS version, assuming Core Audio taps not supported");
            false
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
    
    /// Parse macOS version string into tuple (major, minor, patch)
    fn parse_macos_version(&self, version: &str) -> Result<(u32, u32, u32)> {
        let parts: Vec<&str> = version.split('.').collect();
        
        if parts.len() < 2 {
            return Err(anyhow::anyhow!("Invalid macOS version format: {}", version));
        }
        
        let major = parts[0].parse::<u32>()?;
        let minor = parts[1].parse::<u32>()?;
        let patch = if parts.len() > 2 { 
            parts[2].parse::<u32>().unwrap_or(0) 
        } else { 
            0 
        };
        
        Ok((major, minor, patch))
    }
    
    /// Cleanup resources
    pub fn destroy(&mut self) -> Result<()> {
        self.stop_capture()?;
        
        #[cfg(target_os = "macos")]
        {
            use crate::coreaudio_taps::{destroy_process_tap, format_osstatus_error};
            
            // Destroy process tap if it exists
            if let Some(tap_id) = self.tap_id {
                info!("Destroying Core Audio process tap with ID {}", tap_id);
                
                unsafe {
                    if let Err(status) = destroy_process_tap(tap_id as u32) {
                        let error_msg = format_osstatus_error(status);
                        warn!("Failed to destroy process tap {}: {} ({})", tap_id, error_msg, status);
                        // Don't fail completely, just log the warning
                    } else {
                        info!("Successfully destroyed process tap {}", tap_id);
                    }
                }
                
                self.tap_id = None;
            }
            
            // TODO: Destroy aggregate device if it exists
            if let Some(device_id) = self.aggregate_device_id {
                info!("TODO: Destroy aggregate device with ID {}", device_id);
                // This would call AudioHardwareDestroyAggregateDevice
                self.aggregate_device_id = None;
            }
        }
        
        // Clear audio channel
        self.audio_tx = None;
        
        info!("Destroyed audio tap for {}", self.process_info.name);
        Ok(())
    }
    
    pub fn is_capturing(&self) -> bool {
        self.is_capturing
    }
    
    pub fn get_process_info(&self) -> &ProcessInfo {
        &self.process_info
    }
}

/// High-level manager for application audio capture
pub struct ApplicationAudioManager {
    discovery: Arc<Mutex<ApplicationDiscovery>>,
    active_taps: Arc<RwLock<HashMap<u32, ApplicationAudioTap>>>, // PID -> Tap
    permission_granted: Arc<RwLock<bool>>,
    max_concurrent_captures: usize,
}

impl ApplicationAudioManager {
    pub fn new() -> Self {
        Self {
            discovery: Arc::new(Mutex::new(ApplicationDiscovery::new())),
            active_taps: Arc::new(RwLock::new(HashMap::new())),
            permission_granted: Arc::new(RwLock::new(false)),
            max_concurrent_captures: 4, // Limit to prevent performance issues
        }
    }
    
    /// Check and request audio capture permissions
    pub async fn request_permissions(&self) -> Result<bool> {
        info!("Requesting audio capture permissions...");
        
        #[cfg(target_os = "macos")]
        {
            use crate::tcc_permissions::{get_permission_manager, TccPermissionStatus};
            
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
                    info!("Instructions for enabling permissions:\n{}", 
                        permission_manager.get_permission_instructions());
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
    pub async fn start_capturing_app(&self, pid: u32) -> Result<broadcast::Receiver<Vec<f32>>> {
        // Check permissions
        if !*self.permission_granted.read().await {
            return Err(anyhow::anyhow!("Audio capture permissions not granted"));
        }
        
        // Check concurrent capture limit
        let active_count = self.active_taps.read().await.len();
        if active_count >= self.max_concurrent_captures {
            return Err(anyhow::anyhow!(
                "Maximum concurrent captures reached ({}/{})", 
                active_count, 
                self.max_concurrent_captures
            ));
        }
        
        // Get process info
        let discovery = self.discovery.lock().await;
        let process_info = discovery.get_process_info(pid)
            .ok_or_else(|| anyhow::anyhow!("Process not found: {}", pid))?;
        drop(discovery);
        
        // Create and configure tap
        #[cfg(target_os = "macos")]
        {
            let mut tap = ApplicationAudioTap::new(process_info);
            tap.create_tap().await?;
            let receiver = tap.start_capture()?;
            
            // Store the tap
            self.active_taps.write().await.insert(pid, tap);
            
            info!("Started capturing audio from PID {}", pid);
            Ok(receiver)
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Err(anyhow::anyhow!("Application audio capture is only supported on macOS"))
        }
    }
    
    /// Create a virtual mixer input channel for an application's audio
    /// This integrates application audio capture with the existing mixer system
    pub async fn create_mixer_input_for_app(&self, pid: u32) -> Result<String> {
        // Start capturing from the application
        let _audio_receiver = self.start_capturing_app(pid).await?;
        
        // TODO: Connect the audio receiver to a new mixer input channel
        // This would involve:
        // 1. Creating a new input channel in the mixer
        // 2. Feeding the audio from the receiver into that channel
        // 3. Setting up proper audio format conversion if needed
        // 4. Handling channel routing and effects
        
        // Get process info for naming
        let discovery = self.discovery.lock().await;
        if let Some(process_info) = discovery.get_process_info(pid) {
            let channel_name = format!("App: {}", process_info.name);
            
            info!("Created virtual mixer input '{}' for PID {}", channel_name, pid);
            info!("⚠️ Mixer integration not yet fully implemented");
            
            Ok(channel_name)
        } else {
            Err(anyhow::anyhow!("Process not found: {}", pid))
        }
    }
    
    /// Stop capturing audio from a specific application
    pub async fn stop_capturing_app(&self, pid: u32) -> Result<()> {
        let mut taps = self.active_taps.write().await;
        if let Some(mut tap) = taps.remove(&pid) {
            tap.destroy()?;
            info!("Stopped capturing audio from PID {}", pid);
            Ok(())
        } else {
            Err(anyhow::anyhow!("No active capture for PID {}", pid))
        }
    }
    
    /// Get list of currently active captures
    pub async fn get_active_captures(&self) -> Vec<ProcessInfo> {
        let taps = self.active_taps.read().await;
        taps.values()
            .map(|tap| tap.get_process_info().clone())
            .collect()
    }
    
    /// Stop all active captures
    pub async fn stop_all_captures(&self) -> Result<()> {
        let mut taps = self.active_taps.write().await;
        
        for (pid, mut tap) in taps.drain() {
            if let Err(e) = tap.destroy() {
                error!("Failed to destroy tap for PID {}: {}", pid, e);
            }
        }
        
        info!("Stopped all active audio captures");
        Ok(())
    }
    
    /// Check if permissions are granted
    pub async fn has_permissions(&self) -> bool {
        *self.permission_granted.read().await
    }
}

/// Errors that can occur during application audio operations
#[derive(Debug, thiserror::Error)]
pub enum ApplicationAudioError {
    #[error("Permission denied - audio capture not authorized")]
    PermissionDenied,
    
    #[error("Application not found (PID: {pid})")]
    ApplicationNotFound { pid: u32 },
    
    #[error("Core Audio error: {status}")]
    CoreAudioError { status: i32 },
    
    #[error("Unsupported macOS version - requires 14.4+")]
    UnsupportedSystem,
    
    #[error("Too many active captures (max: {max})")]
    TooManyCaptures { max: usize },
    
    #[error("Audio tap not initialized")]
    TapNotInitialized,
    
    #[error("System error: {0}")]
    SystemError(#[from] anyhow::Error),
}