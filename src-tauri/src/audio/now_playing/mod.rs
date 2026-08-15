// Now-playing metadata for captured applications
//
// Application audio capture delivers PCM and nothing else, so track details are
// read separately by asking Apple Music and Spotify through AppleScript. Polling
// is limited to players the mixer is actually configured to capture.

pub mod applescript;
pub mod configured_inputs;
pub mod media_remote;
pub mod types;
pub mod watcher;

pub use applescript::fetch_now_playing;
pub use configured_inputs::configured_players;
pub use types::{NowPlayingError, NowPlayingTrack, PlayerState, SupportedPlayer};
pub use watcher::{
    NowPlayingErrorEvent, NowPlayingEvent, NowPlayingWatcher, NOW_PLAYING_CHANGED_EVENT,
    NOW_PLAYING_ERROR_EVENT,
};
