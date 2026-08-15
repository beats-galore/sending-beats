// AppleScript-backed track metadata for Apple Music and Spotify.
//
// Neither Core Audio process taps nor ScreenCaptureKit carry metadata alongside
// the PCM they deliver, so track details have to be read out of band by asking
// the player itself through its scripting dictionary.

use colored::Colorize;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::warn;

use super::types::{NowPlayingError, NowPlayingTrack, PlayerState, SupportedPlayer};

/// Delimiter the scripts join their fields with. A unit separator cannot occur
/// in a track title, so splitting on it stays unambiguous where a comma or tab
/// would not.
const FIELD_SEPARATOR: char = '\u{1f}';

/// How many fields each script emits, in the order `parse_track` reads them.
const FIELD_COUNT: usize = 8;

/// A player stuck behind a modal dialog never answers, so give up rather than
/// let the polling task wedge behind it.
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(5);

/// AppleScript's error number for a process denied Automation consent.
const ERR_NOT_AUTHORIZED: &str = "-1743";

/// Both scripts guard their `tell` block with `is running`, which answers
/// without launching the app - and answers `false` rather than erroring when
/// the app is not installed at all.
const APPLE_MUSIC_SCRIPT: &str = r#"
if application "Music" is running then
	tell application "Music"
		set fieldSep to (character id 31)
		try
			set theTrack to current track
		on error
			return ""
		end try
		set playerStatus to "stopped"
		if player state is playing then set playerStatus to "playing"
		if player state is paused then set playerStatus to "paused"
		set playPos to 0
		try
			set thePos to player position
			if thePos is not missing value then set playPos to thePos
		end try
		set trackID to ""
		try
			set trackID to (persistent ID of theTrack) as text
		end try
		return (name of theTrack) & fieldSep & (artist of theTrack) & fieldSep & (album of theTrack) & fieldSep & ((duration of theTrack) as text) & fieldSep & (playPos as text) & fieldSep & playerStatus & fieldSep & trackID & fieldSep & ""
	end tell
else
	return ""
end if
"#;

const SPOTIFY_SCRIPT: &str = r#"
if application "Spotify" is running then
	tell application "Spotify"
		set fieldSep to (character id 31)
		try
			set theTrack to current track
		on error
			return ""
		end try
		set playerStatus to "stopped"
		if player state is playing then set playerStatus to "playing"
		if player state is paused then set playerStatus to "paused"
		set playPos to 0
		try
			set thePos to player position
			if thePos is not missing value then set playPos to thePos
		end try
		set artURL to ""
		try
			set artURL to (artwork url of theTrack) as text
		end try
		set trackID to ""
		try
			set trackID to (id of theTrack) as text
		end try
		return (name of theTrack) & fieldSep & (artist of theTrack) & fieldSep & (album of theTrack) & fieldSep & (((duration of theTrack) / 1000) as text) & fieldSep & (playPos as text) & fieldSep & playerStatus & fieldSep & trackID & fieldSep & artURL
	end tell
else
	return ""
end if
"#;

fn script_for(player: SupportedPlayer) -> &'static str {
    match player {
        SupportedPlayer::AppleMusic => APPLE_MUSIC_SCRIPT,
        SupportedPlayer::Spotify => SPOTIFY_SCRIPT,
    }
}

/// Read the track a player currently has loaded.
///
/// `Ok(None)` covers the player not running and the player having nothing
/// loaded. Both are ordinary states rather than failures, so only a genuinely
/// broken query - denied consent, a hang, a malformed reply - returns `Err`.
pub async fn fetch_now_playing(
    player: SupportedPlayer,
) -> Result<Option<NowPlayingTrack>, NowPlayingError> {
    let raw = run_script(player).await?;
    Ok(parse_track(player, &raw))
}

async fn run_script(player: SupportedPlayer) -> Result<String, NowPlayingError> {
    let child = Command::new("osascript")
        .arg("-e")
        .arg(script_for(player))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    // Dropping the timed-out future drops the child, and `kill_on_drop` reaps it.
    let output = match timeout(SCRIPT_TIMEOUT, child.wait_with_output()).await {
        Ok(result) => result?,
        Err(_) => {
            return Err(NowPlayingError::Timeout {
                app: player.application_name(),
                seconds: SCRIPT_TIMEOUT.as_secs(),
            })
        }
    };

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if stderr.contains(ERR_NOT_AUTHORIZED) {
        return Err(NowPlayingError::AutomationDenied {
            app: player.application_name(),
        });
    }

    Err(NowPlayingError::ScriptFailed(stderr.trim().to_string()))
}

fn parse_track(player: SupportedPlayer, raw: &str) -> Option<NowPlayingTrack> {
    let raw = raw.strip_suffix('\n').unwrap_or(raw);
    if raw.is_empty() {
        return None;
    }

    let fields: Vec<&str> = raw.split(FIELD_SEPARATOR).collect();
    if fields.len() != FIELD_COUNT {
        warn!(
            "{} {} returned {} fields, expected {}",
            "NOW_PLAYING".on_magenta().white(),
            player.application_name(),
            fields.len(),
            FIELD_COUNT
        );
        return None;
    }

    let state = PlayerState::from_script_word(fields[5]);

    Some(NowPlayingTrack {
        bundle_id: player.bundle_id().to_string(),
        title: fields[0].to_string(),
        artist: fields[1].to_string(),
        album: fields[2].to_string(),
        duration_seconds: fields[3].parse().unwrap_or(0.0),
        position_seconds: fields[4].parse().unwrap_or(0.0),
        // The player answered just now, so the reading is current as of now.
        position_taken_at_ms: NowPlayingTrack::now_ms(),
        playback_rate: if state == PlayerState::Playing {
            1.0
        } else {
            0.0
        },
        player_state: state,
        track_id: fields[6].to_string(),
        artwork_url: Some(fields[7].to_string()).filter(|url| !url.is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(fields: &[&str]) -> String {
        fields.join(&FIELD_SEPARATOR.to_string())
    }

    #[test]
    fn a_silent_player_reports_no_track() {
        assert!(parse_track(SupportedPlayer::Spotify, "").is_none());
        assert!(parse_track(SupportedPlayer::Spotify, "\n").is_none());
    }

    #[test]
    fn every_field_lands_in_the_track() {
        let raw = joined(&[
            "Windowlicker",
            "Aphex Twin",
            "Windowlicker",
            "372.5",
            "41.25",
            "playing",
            "spotify:track:abc",
            "https://i.scdn.co/image/abc",
        ]);

        let track = parse_track(SupportedPlayer::Spotify, &raw).expect("track parses");

        assert_eq!(track.bundle_id, "com.spotify.client");
        assert_eq!(track.title, "Windowlicker");
        assert_eq!(track.artist, "Aphex Twin");
        assert_eq!(track.duration_seconds, 372.5);
        assert_eq!(track.position_seconds, 41.25);
        assert_eq!(track.player_state, PlayerState::Playing);
        assert_eq!(track.track_id, "spotify:track:abc");
        assert_eq!(
            track.artwork_url.as_deref(),
            Some("https://i.scdn.co/image/abc")
        );
    }

    #[test]
    fn an_empty_artwork_field_becomes_no_artwork() {
        let raw = joined(&["Song", "Artist", "Album", "10", "0", "paused", "id-1", ""]);

        let track = parse_track(SupportedPlayer::AppleMusic, &raw).expect("track parses");

        assert_eq!(track.artwork_url, None);
        assert_eq!(track.player_state, PlayerState::Paused);
        assert_eq!(track.bundle_id, "com.apple.Music");
    }

    #[test]
    fn a_title_holding_the_delimiters_we_do_not_use_survives_intact() {
        let raw = joined(&[
            "Weird, Title\twith punctuation",
            "Artist",
            "Album",
            "1",
            "0",
            "playing",
            "id-2",
            "",
        ]);

        let track = parse_track(SupportedPlayer::Spotify, &raw).expect("track parses");

        assert_eq!(track.title, "Weird, Title\twith punctuation");
    }

    #[test]
    fn an_apple_music_reply_parses_as_the_player_actually_writes_it() {
        // Apple Music emits a full-precision duration, an advancing position,
        // and no artwork field.
        let raw = joined(&[
            "A Cold Freezin' Night",
            "The Books",
            "The Way Out",
            "202.427001953125",
            "30.29700088501",
            "playing",
            "665FF20C0A8EC9F3",
            "",
        ]);

        let track = parse_track(SupportedPlayer::AppleMusic, &raw).expect("track parses");

        assert_eq!(track.title, "A Cold Freezin' Night");
        assert_eq!(track.artist, "The Books");
        assert_eq!(track.album, "The Way Out");
        assert_eq!(track.duration_seconds, 202.427001953125);
        assert_eq!(track.position_seconds, 30.29700088501);
        assert_eq!(track.player_state, PlayerState::Playing);
        assert_eq!(track.track_id, "665FF20C0A8EC9F3");
        assert_eq!(track.artwork_url, None);
    }

    #[test]
    fn a_moving_playhead_alone_leaves_the_track_state_unchanged() {
        let at = |position: &str| {
            parse_track(
                SupportedPlayer::AppleMusic,
                &joined(&[
                    "A Cold Freezin' Night",
                    "The Books",
                    "The Way Out",
                    "202.427001953125",
                    position,
                    "playing",
                    "665FF20C0A8EC9F3",
                    "",
                ]),
            )
            .expect("track parses")
        };

        assert!(at("30.29700088501").is_same_state(&at("33.54700088501")));
    }

    #[test]
    fn a_truncated_reply_is_rejected_rather_than_half_read() {
        let raw = joined(&["Song", "Artist", "Album"]);

        assert!(parse_track(SupportedPlayer::Spotify, &raw).is_none());
    }

    #[test]
    fn unparseable_timings_fall_back_to_zero_instead_of_dropping_the_track() {
        let raw = joined(&[
            "Song",
            "Artist",
            "Album",
            "missing value",
            "missing value",
            "playing",
            "id-3",
            "",
        ]);

        let track = parse_track(SupportedPlayer::AppleMusic, &raw).expect("track parses");

        assert_eq!(track.duration_seconds, 0.0);
        assert_eq!(track.position_seconds, 0.0);
    }
}
