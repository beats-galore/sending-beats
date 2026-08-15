// Background polling of configured application inputs for track changes.

use colored::Colorize;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{info, warn};

use super::applescript::fetch_now_playing;
use super::configured_inputs::{configured_application_bundles, configured_players};
use super::media_remote::{spawn_stream, stream_lines, AdapterPaths, SessionReader};
use super::types::{NowPlayingTrack, SupportedPlayer};
use crate::db::AudioDatabase;
use std::path::PathBuf;
use std::sync::Mutex as StdMutex;
use tauri::path::BaseDirectory;
use tauri::Manager;

pub const NOW_PLAYING_CHANGED_EVENT: &str = "now-playing-changed";
pub const NOW_PLAYING_ERROR_EVENT: &str = "now-playing-error";

/// A poll costs a few milliseconds of CPU and a couple hundred waiting on the
/// player to answer, so a one second beat is affordable and keeps the playhead
/// readable without the frontend having to guess at it between polls.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingEvent {
    pub bundle_id: String,
    /// `None` once the player stops, quits, or leaves the input configuration,
    /// so a listener knows to clear whatever it was showing.
    pub track: Option<NowPlayingTrack>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingErrorEvent {
    pub bundle_id: String,
    pub message: String,
}

/// What the last poll saw for one player, used to keep the watcher quiet until
/// something actually changes.
#[derive(Default)]
struct PlayerReading {
    track: Option<NowPlayingTrack>,
    error: Option<String>,
}

/// The application the system session currently describes, if any.
///
/// The two sources overlap wherever a player is both scriptable and publishing
/// to the session. MediaRemote carries more for those players - genre, track
/// number, a playhead that needs no polling - so it takes precedence, and the
/// poller stands down for whichever player it is speaking for rather than the
/// two of them overwriting each other.
type SessionOwner = Arc<StdMutex<Option<String>>>;

/// Owns the two tasks that follow what configured inputs are playing.
///
/// They cover different ground and neither subsumes the other: AppleScript can
/// be asked about each player separately but only reaches players with a
/// scripting dictionary, while MediaRemote reaches everything but describes
/// only the one application that currently owns the system session.
#[derive(Default)]
pub struct NowPlayingWatcher {
    task: Option<JoinHandle<()>>,
    session_task: Option<JoinHandle<()>>,
}

impl NowPlayingWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self) -> bool {
        self.task.is_some()
    }

    /// Begin watching, replacing any tasks already running.
    pub fn start(&mut self, app: AppHandle, database: Arc<AudioDatabase>) {
        self.stop();

        info!(
            "{} Watching configured application inputs every {}s",
            "NOW_PLAYING".on_magenta().white(),
            POLL_INTERVAL.as_secs()
        );

        let session_owner: SessionOwner = Arc::new(StdMutex::new(None));

        self.task = Some(tauri::async_runtime::spawn(poll_loop(
            app.clone(),
            database.clone(),
            session_owner.clone(),
        )));
        self.session_task = Some(tauri::async_runtime::spawn(session_loop(
            app,
            database,
            session_owner,
        )));
    }

    pub fn stop(&mut self) {
        let running = self.task.is_some() || self.session_task.is_some();

        for task in [self.task.take(), self.session_task.take()]
            .into_iter()
            .flatten()
        {
            task.abort();
        }

        if running {
            info!("{} Stopped watching", "NOW_PLAYING".on_magenta().white());
        }
    }
}

impl Drop for NowPlayingWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Where the bundled adapter sits, or where it sits in the source tree when the
/// app is running from `tauri dev` and has no `Resources` directory yet.
fn adapter_paths(app: &AppHandle) -> AdapterPaths {
    let bundled = |relative: &str| {
        app.path()
            .resolve(relative, BaseDirectory::Resource)
            .ok()
            .filter(|path| path.exists())
    };

    let source = |relative: &str| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(relative)
    };

    AdapterPaths {
        script: bundled("_up_/src-mediaremote/bin/mediaremote-adapter.pl")
            .unwrap_or_else(|| source("src-mediaremote/bin/mediaremote-adapter.pl")),
        framework: bundled("_up_/src-mediaremote/build/MediaRemoteAdapter.framework")
            .unwrap_or_else(|| source("src-mediaremote/build/MediaRemoteAdapter.framework")),
    }
}

/// Follow the system now-playing session, reporting it against whichever
/// configured input it belongs to.
///
/// This covers any application at all, including the scriptable ones, and takes
/// precedence over the poller for whichever it is currently describing. What it
/// cannot do is describe two at once - the session only ever names one - which
/// is why the poller still runs for everything else.
async fn session_loop(app: AppHandle, database: Arc<AudioDatabase>, owner: SessionOwner) {
    let paths = adapter_paths(&app);

    let mut child = match spawn_stream(&paths) {
        Ok(child) => child,
        Err(error) => {
            warn!(
                "{} System session unavailable: {}",
                "NOW_PLAYING".on_magenta().white(),
                error
            );
            return;
        }
    };

    let Some(mut lines) = stream_lines(&mut child) else {
        warn!(
            "{} Adapter produced no output stream",
            "NOW_PLAYING".on_magenta().white()
        );
        return;
    };

    info!(
        "{} Following the system now-playing session",
        "NOW_PLAYING".on_magenta().white()
    );

    // `child` stays owned by this task and nothing else. Aborting the task drops
    // it, and `kill_on_drop` reaps the adapter - which is the only thing that
    // stops a perl process outliving the app that started it.
    let mut session = SessionReader::new();
    let mut reported: Option<NowPlayingTrack> = None;

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        let track = session.accept(&line);

        // Only speak for an application the mixer is actually capturing.
        let configured = match configured_application_bundles(database.sea_orm()).await {
            Ok(bundles) => bundles,
            Err(error) => {
                warn!(
                    "{} Could not read configured inputs: {}",
                    "NOW_PLAYING".on_magenta().white(),
                    error
                );
                continue;
            }
        };

        let track = track.filter(|track| configured.contains(&track.bundle_id));

        // Claimed before emitting, so the poller has already stood down by the
        // time this reading reaches a listener.
        if let Ok(mut owner) = owner.lock() {
            *owner = track.as_ref().map(|track| track.bundle_id.clone());
        }

        if reported.as_ref().map(|previous| previous.bundle_id.clone())
            != track.as_ref().map(|current| current.bundle_id.clone())
        {
            clear_previous_session(&app, reported.as_ref(), track.as_ref());
        }

        if reported == track {
            continue;
        }

        if !is_same_reading(&reported, &track) {
            log_session_change(&track);
        }
        reported = track.clone();

        if let Some(current) = track {
            emit(
                &app,
                NOW_PLAYING_CHANGED_EVENT,
                NowPlayingEvent {
                    bundle_id: current.bundle_id.clone(),
                    track: Some(current),
                },
            );
        }
    }
}

/// Tell listeners the application that just lost the session has nothing playing.
fn clear_previous_session(
    app: &AppHandle,
    previous: Option<&NowPlayingTrack>,
    current: Option<&NowPlayingTrack>,
) {
    let Some(previous) = previous else {
        return;
    };

    if current.is_some_and(|track| track.bundle_id == previous.bundle_id) {
        return;
    }

    emit(
        app,
        NOW_PLAYING_CHANGED_EVENT,
        NowPlayingEvent {
            bundle_id: previous.bundle_id.clone(),
            track: None,
        },
    );
}

fn log_session_change(track: &Option<NowPlayingTrack>) {
    match track {
        Some(track) => info!(
            "{} {}: {} - {} [{:?}]",
            "NOW_PLAYING".on_magenta().white(),
            track.bundle_id,
            track.artist,
            track.title,
            track.player_state
        ),
        None => info!(
            "{} System session: nothing playing",
            "NOW_PLAYING".on_magenta().white()
        ),
    }
}

async fn poll_loop(app: AppHandle, database: Arc<AudioDatabase>, owner: SessionOwner) {
    let mut ticker = interval(POLL_INTERVAL);
    // A player slow to answer should push the next poll back, not bank missed
    // ticks and fire them off back to back once it recovers.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut readings: HashMap<SupportedPlayer, PlayerReading> = HashMap::new();
    let mut last_lookup_error: Option<String> = None;

    loop {
        ticker.tick().await;

        let players = match configured_players(database.sea_orm()).await {
            Ok(players) => {
                last_lookup_error = None;
                players
            }
            Err(error) => {
                let message = error.to_string();
                if last_lookup_error.as_deref() != Some(message.as_str()) {
                    warn!(
                        "{} Could not read configured inputs: {}",
                        "NOW_PLAYING".on_magenta().white(),
                        message
                    );
                    last_lookup_error = Some(message);
                }
                continue;
            }
        };

        forget_unconfigured(&app, &mut readings, &players);

        let session_owner = owner.lock().ok().and_then(|owner| owner.clone());

        for player in players {
            // The session is already describing this one, and in more detail
            // than a script can. Forget what was polled so that handing the
            // player back produces a fresh reading rather than a stale match.
            if session_owner.as_deref() == Some(player.bundle_id()) {
                readings.remove(&player);
                continue;
            }

            poll_player(&app, player, &mut readings).await;
        }
    }
}

async fn poll_player(
    app: &AppHandle,
    player: SupportedPlayer,
    readings: &mut HashMap<SupportedPlayer, PlayerReading>,
) {
    let reading = readings.entry(player).or_default();

    match fetch_now_playing(player).await {
        Ok(track) => {
            reading.error = None;
            // Compared in full, playhead included, so the frontend can follow the
            // position rather than interpolate it. Logging is the narrower test:
            // a track that has only advanced is not worth a line every second.
            if reading.track == track {
                return;
            }

            if !is_same_reading(&reading.track, &track) {
                log_change(player, &track);
            }
            reading.track = track.clone();
            emit(
                app,
                NOW_PLAYING_CHANGED_EVENT,
                NowPlayingEvent {
                    bundle_id: player.bundle_id().to_string(),
                    track,
                },
            );
        }
        Err(error) => {
            let message = error.to_string();
            if reading.error.as_deref() == Some(message.as_str()) {
                return;
            }

            warn!(
                "{} {}: {}",
                "NOW_PLAYING".on_magenta().white(),
                player.display_name(),
                message
            );
            reading.error = Some(message.clone());
            emit(
                app,
                NOW_PLAYING_ERROR_EVENT,
                NowPlayingErrorEvent {
                    bundle_id: player.bundle_id().to_string(),
                    message,
                },
            );
        }
    }
}

/// Drop readings for players that have left the input configuration, clearing
/// anything a listener still has on screen for them.
fn forget_unconfigured(
    app: &AppHandle,
    readings: &mut HashMap<SupportedPlayer, PlayerReading>,
    configured: &[SupportedPlayer],
) {
    let dropped: Vec<SupportedPlayer> = readings
        .keys()
        .filter(|player| !configured.contains(player))
        .copied()
        .collect();

    for player in dropped {
        let was_reporting_a_track = readings
            .remove(&player)
            .is_some_and(|reading| reading.track.is_some());

        if was_reporting_a_track {
            emit(
                app,
                NOW_PLAYING_CHANGED_EVENT,
                NowPlayingEvent {
                    bundle_id: player.bundle_id().to_string(),
                    track: None,
                },
            );
        }
    }
}

fn is_same_reading(previous: &Option<NowPlayingTrack>, current: &Option<NowPlayingTrack>) -> bool {
    match (previous, current) {
        (None, None) => true,
        (Some(previous), Some(current)) => previous.is_same_state(current),
        _ => false,
    }
}

fn log_change(player: SupportedPlayer, track: &Option<NowPlayingTrack>) {
    match track {
        Some(track) => info!(
            "{} {}: {} - {} [{:?}]",
            "NOW_PLAYING".on_magenta().white(),
            player.display_name(),
            track.artist,
            track.title,
            track.player_state
        ),
        None => info!(
            "{} {}: nothing playing",
            "NOW_PLAYING".on_magenta().white(),
            player.display_name()
        ),
    }
}

fn emit<P: Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(error) = app.emit(event, payload) {
        warn!(
            "{} Failed to emit {}: {}",
            "NOW_PLAYING".on_magenta().white(),
            event,
            error
        );
    }
}
