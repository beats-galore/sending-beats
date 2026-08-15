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
use super::configured_inputs::configured_players;
use super::types::{NowPlayingTrack, SupportedPlayer};
use crate::db::AudioDatabase;

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

/// Owns the polling task that watches whichever supported players are
/// configured as inputs.
#[derive(Default)]
pub struct NowPlayingWatcher {
    task: Option<JoinHandle<()>>,
}

impl NowPlayingWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self) -> bool {
        self.task.is_some()
    }

    /// Begin polling, replacing any task already running.
    pub fn start(&mut self, app: AppHandle, database: Arc<AudioDatabase>) {
        self.stop();

        info!(
            "{} Watching configured application inputs every {}s",
            "NOW_PLAYING".on_magenta().white(),
            POLL_INTERVAL.as_secs()
        );

        self.task = Some(tauri::async_runtime::spawn(poll_loop(app, database)));
    }

    pub fn stop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            info!("{} Stopped watching", "NOW_PLAYING".on_magenta().white());
        }
    }
}

impl Drop for NowPlayingWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn poll_loop(app: AppHandle, database: Arc<AudioDatabase>) {
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

        for player in players {
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
