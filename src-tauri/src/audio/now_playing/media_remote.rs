// The system now-playing session, read through the bundled MediaRemote adapter.
//
// AppleScript can only reach players that publish a scripting dictionary, which
// rules out browsers, QuickTime, Podcasts and most everything else. MediaRemote
// knows about all of them, but Apple restricts reading it to its own binaries,
// so the query runs out of /usr/bin/perl - a platform binary - loading a helper
// framework that this app ships.
//
// The tradeoff against AppleScript is coverage for exclusivity: MediaRemote
// describes whichever single application currently owns the session, where
// AppleScript can be asked about each player independently.

use serde::Deserialize;
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use super::types::{NowPlayingError, NowPlayingTrack, PlayerState};

/// Platform binary Apple trusts to talk to the media daemon.
const PERL: &str = "/usr/bin/perl";

/// One frame of the adapter's stream.
///
/// A frame with `diff` set carries only the keys that changed, so frames have
/// to be folded onto the running payload rather than replacing it.
#[derive(Debug, Deserialize)]
struct StreamFrame {
    #[serde(default)]
    diff: bool,
    #[serde(default)]
    payload: Map<String, Value>,
}

/// Where the adapter lives, which differs between a bundled app and a dev run.
#[derive(Debug, Clone)]
pub struct AdapterPaths {
    pub script: PathBuf,
    pub framework: PathBuf,
}

impl AdapterPaths {
    pub fn exists(&self) -> bool {
        self.script.is_file() && self.framework.is_dir()
    }
}

/// Start the adapter streaming now-playing changes.
///
/// Artwork is excluded deliberately: the adapter embeds it as base64 and a
/// single frame runs to several hundred kilobytes with it left in.
pub fn spawn_stream(paths: &AdapterPaths) -> Result<Child, NowPlayingError> {
    if !paths.exists() {
        return Err(NowPlayingError::AdapterMissing {
            path: paths.framework.display().to_string(),
        });
    }

    Command::new(PERL)
        .arg(&paths.script)
        .arg(&paths.framework)
        .arg("stream")
        .arg("--no-artwork")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(NowPlayingError::Spawn)
}

/// Reads frames off the adapter, folding diffs into the session state.
#[derive(Default)]
pub struct SessionReader {
    state: Map<String, Value>,
}

impl SessionReader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply one line of adapter output, returning the session it now describes.
    ///
    /// `Ok(None)` covers both a line that is not a frame and a session with
    /// nothing in it, which is what an empty payload means.
    pub fn accept(&mut self, line: &str) -> Option<NowPlayingTrack> {
        let frame: StreamFrame = serde_json::from_str(line).ok()?;

        if frame.diff {
            self.state.extend(frame.payload);
        } else {
            self.state = frame.payload;
        }

        self.track()
    }

    fn track(&self) -> Option<NowPlayingTrack> {
        let bundle_id = self.string("bundleIdentifier");
        let title = self.string("title");

        // An empty payload is the adapter saying nothing owns the session.
        if bundle_id.is_empty() || title.is_empty() {
            return None;
        }

        let playback_rate = self.number("playbackRate");
        let playing = self
            .state
            .get("playing")
            .and_then(Value::as_bool)
            .unwrap_or(playback_rate > 0.0);

        Some(NowPlayingTrack {
            bundle_id,
            title,
            artist: self.string("artist"),
            album: self.string("album"),
            duration_seconds: self.number("duration"),
            position_seconds: self.number("elapsedTime"),
            position_taken_at_ms: self.timestamp_ms(),
            playback_rate,
            player_state: if playing {
                PlayerState::Playing
            } else {
                PlayerState::Paused
            },
            // MediaRemote carries artwork as bytes, which `--no-artwork` drops.
            artwork_url: None,
            track_id: self.string("contentItemIdentifier"),
        })
    }

    fn string(&self, key: &str) -> String {
        self.state
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    fn number(&self, key: &str) -> f64 {
        self.state.get(key).and_then(Value::as_f64).unwrap_or(0.0)
    }

    /// When the playhead reading was taken, as epoch milliseconds.
    ///
    /// Falls back to now if the frame carried no timestamp, which ages the
    /// position from this moment rather than from an unknown one.
    fn timestamp_ms(&self) -> i64 {
        self.state
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|stamp| chrono::DateTime::parse_from_rfc3339(stamp).ok())
            .map(|stamp| stamp.timestamp_millis())
            .unwrap_or_else(NowPlayingTrack::now_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"{"type":"data","diff":false,"payload":{"artist":"clumsy342","playbackRate":1,"title":"Animal Collective - Brother Sport","elapsedTime":0.016,"duration":432.88,"playing":true,"bundleIdentifier":"com.brave.Browser","album":"","timestamp":"2026-08-15T11:26:01Z"}}"#;

    #[test]
    fn an_empty_session_reports_no_track() {
        let mut reader = SessionReader::new();

        assert!(reader
            .accept(r#"{"type":"data","diff":false,"payload":{}}"#)
            .is_none());
    }

    #[test]
    fn a_browser_session_carries_the_media_session_metadata() {
        let mut reader = SessionReader::new();

        let track = reader.accept(FULL).expect("frame parses");

        assert_eq!(track.bundle_id, "com.brave.Browser");
        assert_eq!(track.title, "Animal Collective - Brother Sport");
        assert_eq!(track.artist, "clumsy342");
        assert_eq!(track.duration_seconds, 432.88);
        assert_eq!(track.playback_rate, 1.0);
        assert_eq!(track.player_state, PlayerState::Playing);
        // 2026-08-15T11:26:01Z
        assert_eq!(track.position_taken_at_ms, 1786793161000);
    }

    #[test]
    fn a_diff_frame_changes_only_what_it_names() {
        let mut reader = SessionReader::new();
        reader.accept(FULL).expect("full frame parses");

        let track = reader
            .accept(r#"{"type":"data","diff":true,"payload":{"playing":false,"playbackRate":0}}"#)
            .expect("diff frame parses");

        // Everything the diff did not mention has to survive it.
        assert_eq!(track.title, "Animal Collective - Brother Sport");
        assert_eq!(track.artist, "clumsy342");
        assert_eq!(track.duration_seconds, 432.88);
        assert_eq!(track.player_state, PlayerState::Paused);
        assert_eq!(track.playback_rate, 0.0);
    }

    #[test]
    fn a_full_frame_drops_what_it_omits() {
        let mut reader = SessionReader::new();
        reader.accept(FULL).expect("full frame parses");

        let replaced = reader.accept(
            r#"{"type":"data","diff":false,"payload":{"title":"Other","bundleIdentifier":"com.apple.Music","playing":true,"playbackRate":1}}"#,
        ).expect("frame parses");

        assert_eq!(replaced.title, "Other");
        assert_eq!(replaced.artist, "");
        assert_eq!(replaced.duration_seconds, 0.0);
    }

    #[test]
    fn playing_falls_back_to_the_rate_when_the_flag_is_absent() {
        let mut reader = SessionReader::new();

        let track = reader
            .accept(
                r#"{"type":"data","diff":false,"payload":{"title":"T","bundleIdentifier":"b","playbackRate":1}}"#,
            )
            .expect("frame parses");

        assert_eq!(track.player_state, PlayerState::Playing);
    }

    #[test]
    fn a_line_that_is_not_a_frame_is_ignored() {
        let mut reader = SessionReader::new();

        assert!(reader.accept("not json").is_none());
    }
}

/// Read `stream` output line by line, handing each parsed session to `on_change`.
pub async fn read_stream<F>(child: &mut Child, mut on_change: F)
where
    F: FnMut(Option<NowPlayingTrack>),
{
    let Some(stdout) = child.stdout.take() else {
        return;
    };

    let mut reader = SessionReader::new();
    let mut lines = BufReader::new(stdout).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        on_change(reader.accept(&line));
    }
}
