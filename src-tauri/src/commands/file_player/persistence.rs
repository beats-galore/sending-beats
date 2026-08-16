// Keeping a running player and its stored row in step
//
// The player knows what it is doing and the database knows what it did. These
// are the two conversions between them, plus the task that listens for tracks
// finishing so history is written without the decoding thread ever waiting on a
// database.

use std::sync::Arc;
use std::time::Duration;

use crate::audio::{AudioFilePlayer, PlayerEvent, QueuedTrack, RepeatMode};
use crate::db::FilePlayerStore;
use crate::entities::file_player_track;

/// Turn a stored track back into one the player can queue
///
/// The row's key becomes the track's id, which is what lets a track finishing be
/// written back against the row it came from.
pub fn queued_track_from_row(row: file_player_track::Model) -> QueuedTrack {
    QueuedTrack {
        id: row.id,
        file_path: std::path::PathBuf::from(row.file_path),
        title: row.title,
        artist: row.artist,
        album: row.album,
        duration: row
            .duration_ms
            .and_then(|ms| u64::try_from(ms).ok())
            .map(Duration::from_millis),
        file_size: u64::try_from(row.file_size).unwrap_or(0),
        added_at: row.created_at,
    }
}

/// Read a stored repeat mode, falling back to playing through once
///
/// An unknown value is a row written by a version that knew a mode this one does
/// not, and refusing to load the player over it would be worse than playing the
/// queue straight through.
pub fn repeat_mode_from(stored: &str) -> RepeatMode {
    match stored {
        "track" => RepeatMode::Track,
        "queue" => RepeatMode::Queue,
        _ => RepeatMode::None,
    }
}

/// The name a repeat mode is stored under
pub fn repeat_mode_name(mode: RepeatMode) -> &'static str {
    match mode {
        RepeatMode::None => "none",
        RepeatMode::Track => "track",
        RepeatMode::Queue => "queue",
    }
}

/// Listen for tracks finishing and write them into the play log
///
/// The decoding thread reports a finished track down a channel and carries on;
/// this drains it on the runtime, so writing the log never happens on the thread
/// that has audio to produce. It ends when the player is dropped and its sender
/// goes with it.
///
/// The track stays in its queue. A queue is a list that was built on purpose,
/// not something that empties as it is used, so playing writes a row beside it
/// rather than taking it out.
pub fn spawn_history_writer(player: &Arc<AudioFilePlayer>, database: Arc<crate::AudioDatabase>) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PlayerEvent>();
    player.set_event_sender(tx);

    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                PlayerEvent::TrackFinished { track_id } => {
                    if let Err(e) =
                        FilePlayerStore::record_play(database.sea_orm(), &track_id).await
                    {
                        tracing::warn!("Could not record that track '{}' played: {}", track_id, e);
                    }
                }
            }
        }
    });
}
