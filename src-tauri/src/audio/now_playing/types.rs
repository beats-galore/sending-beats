// Types shared between the AppleScript providers and the polling watcher.

use serde::{Deserialize, Serialize};

/// Media players this module can read track metadata from.
///
/// Each variant ties a bundle identifier - the same one application audio
/// capture keys off - to the name the app answers to in an AppleScript `tell`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SupportedPlayer {
    AppleMusic,
    Spotify,
}

impl SupportedPlayer {
    pub const ALL: [SupportedPlayer; 2] = [SupportedPlayer::AppleMusic, SupportedPlayer::Spotify];

    pub fn from_bundle_id(bundle_id: &str) -> Option<Self> {
        match bundle_id {
            "com.apple.Music" => Some(Self::AppleMusic),
            "com.spotify.client" => Some(Self::Spotify),
            _ => None,
        }
    }

    pub fn bundle_id(&self) -> &'static str {
        match self {
            Self::AppleMusic => "com.apple.Music",
            Self::Spotify => "com.spotify.client",
        }
    }

    /// The name the application answers to inside a `tell application` block,
    /// which is not always what the app calls itself to users.
    pub fn application_name(&self) -> &'static str {
        match self {
            Self::AppleMusic => "Music",
            Self::Spotify => "Spotify",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::AppleMusic => "Apple Music",
            Self::Spotify => "Spotify",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlayerState {
    Playing,
    Paused,
    Stopped,
}

impl PlayerState {
    /// Maps the status word the scripts emit. Both players expose the same
    /// three states under different dictionary constants, so the scripts
    /// normalise to these words rather than coercing an enum to text.
    pub fn from_script_word(word: &str) -> Self {
        match word {
            "playing" => Self::Playing,
            "paused" => Self::Paused,
            _ => Self::Stopped,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingTrack {
    pub bundle_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_seconds: f64,
    /// Where the playhead was at `position_taken_at_ms`, not where it is now.
    pub position_seconds: f64,
    /// Epoch milliseconds at which `position_seconds` was true.
    ///
    /// MediaRemote reports a snapshot rather than a live value and republishes
    /// it only when playback changes, so a reader has to age it:
    /// `position_seconds + elapsed_since * playback_rate`. Taking the snapshot
    /// at face value leaves the playhead pinned wherever it was last announced.
    pub position_taken_at_ms: i64,
    /// Zero while paused, one at normal speed. Multiplying the aged position by
    /// this stops a paused playhead from drifting forwards.
    pub playback_rate: f64,
    pub player_state: PlayerState,
    /// Spotify only; Apple Music exposes artwork as image data rather than a URL.
    pub artwork_url: Option<String>,
    /// Persistent ID for Apple Music, track URI for Spotify.
    pub track_id: String,
}

impl NowPlayingTrack {
    /// Whether two readings describe the same track played the same way.
    ///
    /// Position is excluded deliberately, so that a playhead simply advancing
    /// reads as the same state. Callers that care about the playhead compare
    /// the whole reading instead.
    pub fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_millis() as i64)
            .unwrap_or(0)
    }

    pub fn is_same_state(&self, other: &Self) -> bool {
        self.bundle_id == other.bundle_id
            && self.track_id == other.track_id
            && self.title == other.title
            && self.artist == other.artist
            && self.album == other.album
            && self.player_state == other.player_state
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NowPlayingError {
    #[error("No now-playing support for bundle identifier '{bundle_id}'")]
    UnsupportedPlayer { bundle_id: String },

    #[error(
        "Automation permission denied for {app}. Grant Sendin Beats access under \
         System Settings > Privacy & Security > Automation"
    )]
    AutomationDenied { app: &'static str },

    #[error("Timed out after {seconds}s asking {app} for its current track")]
    Timeout { app: &'static str, seconds: u64 },

    #[error("osascript failed: {0}")]
    ScriptFailed(String),

    #[error("MediaRemote adapter not found at {path} - run `make native`")]
    AdapterMissing { path: String },

    #[error("Could not run osascript: {0}")]
    Spawn(#[from] std::io::Error),
}
