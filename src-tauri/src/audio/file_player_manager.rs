use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};

use super::file_player::{AudioFilePlayer, FilePlayerDevice, PlaybackStatus, QueuedTrack};
use super::types::AudioDeviceInfo;

/// Configuration for a file player instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePlayerConfig {
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub auto_play_next: bool,
    pub volume: f32,
}

impl Default for FilePlayerConfig {
    fn default() -> Self {
        Self {
            name: "Media Player".to_string(),
            sample_rate: 48000,
            channels: 2,
            auto_play_next: true,
            volume: 1.0,
        }
    }
}

/// Manages multiple file player instances
pub struct FilePlayerManager {
    players: Arc<Mutex<HashMap<String, Arc<FilePlayerDevice>>>>,
    next_player_id: Arc<Mutex<u32>>,
}

impl FilePlayerManager {
    pub fn new() -> Self {
        Self {
            players: Arc::new(Mutex::new(HashMap::new())),
            next_player_id: Arc::new(Mutex::new(1)),
        }
    }
    
    /// Create a new file player device
    pub fn create_player(&self, config: FilePlayerConfig) -> Result<String> {
        let player_id = {
            let mut next_id = self.next_player_id.lock().unwrap();
            let id = format!("file_player_{}", *next_id);
            *next_id += 1;
            id
        };
        
        let device_name = format!("{} (File Player)", config.name);
        let device = Arc::new(FilePlayerDevice::new(
            device_name,
            config.sample_rate,
            config.channels,
        ));
        
        // Set initial volume
        device.get_player().set_volume(config.volume);
        
        // Store the device
        {
            let mut players = self.players.lock().unwrap();
            players.insert(player_id.clone(), device);
        }
        
        println!("🎵 Created file player: {} ({})", config.name, player_id);
        Ok(player_id)
    }
    
    /// Remove a file player device
    pub fn remove_player(&self, player_id: &str) -> Result<()> {
        let mut players = self.players.lock().unwrap();
        
        if let Some(device) = players.remove(player_id) {
            // Stop playback before removing
            device.get_player().stop();
            println!("🗑️ Removed file player: {}", player_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("File player not found: {}", player_id))
        }
    }
    
    /// Get a file player device by ID
    pub fn get_player(&self, player_id: &str) -> Option<Arc<FilePlayerDevice>> {
        let players = self.players.lock().unwrap();
        players.get(player_id).cloned()
    }
    
    /// Get all file player devices as audio device info
    pub fn get_devices(&self) -> Vec<AudioDeviceInfo> {
        let players = self.players.lock().unwrap();
        
        players.iter().map(|(id, device)| {
            AudioDeviceInfo {
                id: device.get_device_id().to_string(),
                name: device.get_device_name().to_string(),
                is_input: true,
                is_output: false,
                is_default: false,
                supported_sample_rates: vec![48000, 44100], // Common rates
                supported_channels: vec![2], // Stereo
                host_api: "file_player".to_string(),
            }
        }).collect()
    }
    
    /// Get list of all player IDs and names
    pub fn list_players(&self) -> Vec<(String, String)> {
        let players = self.players.lock().unwrap();
        
        players.iter().map(|(id, device)| {
            (id.clone(), device.get_device_name().to_string())
        }).collect()
    }
    
    /// Add track to a specific player's queue
    pub async fn add_track_to_player<P: AsRef<Path>>(&self, player_id: &str, file_path: P) -> Result<String> {
        let device = self.get_player(player_id)
            .context("File player not found")?;
        
        device.get_player().add_track(file_path).await
    }
    
    /// Remove track from a specific player's queue
    pub fn remove_track_from_player(&self, player_id: &str, track_id: &str) -> Result<()> {
        let device = self.get_player(player_id)
            .context("File player not found")?;
        
        device.get_player().remove_track(track_id)
    }
    
    /// Control playback for a specific player
    pub async fn control_player(&self, player_id: &str, action: PlaybackAction) -> Result<()> {
        let device = self.get_player(player_id)
            .context("File player not found")?;
        
        let player = device.get_player();
        
        match action {
            PlaybackAction::Play => player.play().await?,
            PlaybackAction::Pause => player.pause(),
            PlaybackAction::Stop => player.stop(),
            PlaybackAction::SkipNext => player.skip_next().await?,
            PlaybackAction::SkipPrevious => player.skip_previous().await?,
            PlaybackAction::SetVolume(volume) => player.set_volume(volume),
        }
        
        Ok(())
    }
    
    /// Get playback status for a specific player
    pub fn get_player_status(&self, player_id: &str) -> Result<PlaybackStatus> {
        let device = self.get_player(player_id)
            .context("File player not found")?;
        
        Ok(device.get_player().get_status())
    }
    
    /// Get queue for a specific player
    pub fn get_player_queue(&self, player_id: &str) -> Result<Vec<QueuedTrack>> {
        let device = self.get_player(player_id)
            .context("File player not found")?;
        
        Ok(device.get_player().get_queue())
    }
    
    /// Clear queue for a specific player
    pub fn clear_player_queue(&self, player_id: &str) -> Result<()> {
        let device = self.get_player(player_id)
            .context("File player not found")?;
        
        let player = device.get_player();
        
        // Stop playback first
        player.stop();
        
        // Get all track IDs and remove them
        let queue = player.get_queue();
        for track in queue {
            let _ = player.remove_track(&track.id); // Ignore errors
        }
        
        println!("🧹 Cleared queue for player: {}", player_id);
        Ok(())
    }
}

/// Actions that can be performed on a file player
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlaybackAction {
    Play,
    Pause,
    Stop,
    SkipNext,
    SkipPrevious,
    SetVolume(f32),
}

/// File player management for the audio system
pub struct FilePlayerService {
    manager: Arc<FilePlayerManager>,
}

impl FilePlayerService {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(FilePlayerManager::new()),
        }
    }
    
    pub fn get_manager(&self) -> Arc<FilePlayerManager> {
        self.manager.clone()
    }
}

impl Default for FilePlayerService {
    fn default() -> Self {
        Self::new()
    }
}