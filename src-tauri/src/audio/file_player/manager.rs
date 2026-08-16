use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::player::{AudioFilePlayer, FilePlayerDevice, PlaybackStatus, QueuedTrack};
use crate::audio::types::AudioDeviceInfo;

/// Configuration for a file player instance
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
            sample_rate: crate::types::DEFAULT_SAMPLE_RATE,
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

        self.create_player_with_id(player_id, config)
    }

    /// Create a player under an identifier chosen by the caller
    ///
    /// Used where the player already exists on disk: its row key is the identity
    /// everything else refers to, and minting a second one here would leave the
    /// running player and the stored one unable to find each other.
    pub fn create_player_with_id(
        &self,
        player_id: String,
        config: FilePlayerConfig,
    ) -> Result<String> {
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

        players
            .iter()
            .map(|(id, device)| {
                AudioDeviceInfo {
                    // The key, not the device's own uuid: this is the identifier
                    // `create_player` handed back and `get_player` answers to, so
                    // a source picked from this list can be attached.
                    id: id.clone(),
                    name: device.get_device_name().to_string(),
                    uid: None,
                    is_input: true,
                    is_output: false,
                    is_default: false,
                    supported_sample_rates: crate::types::SUPPORTED_SAMPLE_RATES_HZ.to_vec(), // Common rates
                    supported_channels: vec![2], // Stereo
                    host_api: "file_player".to_string(),
                }
            })
            .collect()
    }

    /// Get list of all player IDs and names
    pub fn list_players(&self) -> Vec<(String, String)> {
        let players = self.players.lock().unwrap();

        players
            .iter()
            .map(|(id, device)| (id.clone(), device.get_device_name().to_string()))
            .collect()
    }

    /// Add track to a specific player's queue
    pub async fn add_track_to_player<P: AsRef<Path>>(
        &self,
        player_id: &str,
        file_path: P,
    ) -> Result<String> {
        let device = self
            .get_player(player_id)
            .context("File player not found")?;

        device.get_player().add_track(file_path).await
    }

    /// Remove track from a specific player's queue
    pub fn remove_track_from_player(&self, player_id: &str, track_id: &str) -> Result<()> {
        let device = self
            .get_player(player_id)
            .context("File player not found")?;

        device.get_player().remove_track(track_id)
    }

    /// Control playback for a specific player
    pub async fn control_player(&self, player_id: &str, action: PlaybackAction) -> Result<()> {
        let device = self
            .get_player(player_id)
            .context("File player not found")?;

        let player = device.get_player();

        match action {
            PlaybackAction::Play => player.play()?,
            PlaybackAction::Pause => player.pause(),
            PlaybackAction::Stop => player.stop(),
            PlaybackAction::SkipNext => player.skip_next()?,
            PlaybackAction::SkipPrevious => player.skip_previous()?,
            PlaybackAction::RestartTrack => player.restart_track()?,
            PlaybackAction::PlayTrack { track_id } => player.play_track(&track_id)?,
            PlaybackAction::Seek { seconds } => {
                player.seek(std::time::Duration::from_secs_f64(seconds.max(0.0)))?
            }
            PlaybackAction::SetVolume { volume } => player.set_volume(volume),
        }

        Ok(())
    }

    /// Get playback status for a specific player
    pub fn get_player_status(&self, player_id: &str) -> Result<PlaybackStatus> {
        let device = self
            .get_player(player_id)
            .context("File player not found")?;

        Ok(device.get_player().get_status())
    }

    /// Get queue for a specific player
    pub fn get_player_queue(&self, player_id: &str) -> Result<Vec<QueuedTrack>> {
        let device = self
            .get_player(player_id)
            .context("File player not found")?;

        Ok(device.get_player().get_queue())
    }

    /// Clear queue for a specific player
    pub fn clear_player_queue(&self, player_id: &str) -> Result<()> {
        let device = self
            .get_player(player_id)
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
///
/// Tagged by `type`, so the transport on the other side sends
/// `{ type: 'seek', seconds }` rather than serde's default shape for an enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PlaybackAction {
    Play,
    Pause,
    Stop,
    SkipNext,
    /// Back to the start of this track, or to the one before it — see
    /// `AudioFilePlayer::skip_previous` for which
    SkipPrevious,
    /// This track again from the beginning, whatever the playhead says
    RestartTrack,
    /// Jump to a given track and play it, wherever it sits in the queue
    #[serde(rename_all = "camelCase")]
    PlayTrack {
        track_id: String,
    },
    /// Move the playhead within the current track
    Seek {
        seconds: f64,
    },
    SetVolume {
        volume: f32,
    },
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
