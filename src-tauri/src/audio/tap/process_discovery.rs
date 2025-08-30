// Process discovery and audio application detection
//
// This module handles discovering and tracking audio-capable applications
// on the system, maintaining a registry of known audio apps and scanning
// for running processes.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use sysinfo::{System, Pid};
use tracing::{info, debug};

use super::types::ProcessInfo;

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
        // Disable caching for now - always do a fresh scan
        // TODO: Implement proper caching with stored results later
        
        println!("🔍 SCANNING: Starting audio application scan...");
        self.system.refresh_all();
        self.last_scan = std::time::Instant::now();
        
        let mut audio_processes = Vec::new();
        
        // Iterate through all running processes
        for (pid, process) in self.system.processes() {
            let process_name = process.name();
            
            // Check if this is a known audio application (exact match)
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
        // TODO: Implement proper caching mechanism with stored results
        // For now, return empty vec since caching is disabled
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