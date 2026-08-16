// Editing a queue while it is playing
//
// Reordering and removing are not just list operations here: the player holds
// positions into the queue — which track is playing, and which were played
// before it — and a list that shifts under those leaves the wrong row marked as
// playing and sends "previous" to a track nobody heard.
//
// So every edit in this module comes in two halves: the change to the list, and
// the same change applied to everything pointing into it. They are kept together
// deliberately, because splitting them is exactly how the two drift apart.

use anyhow::Result;

use super::player::AudioFilePlayer;

/// What came of trying to move on from the current track
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
    /// Something new is loaded and decoding can carry on
    Loaded,
    /// Held at a breakpoint, with the next track cued at its start
    Paused,
    /// Nothing left to play
    Exhausted,
}

impl AudioFilePlayer {
    /// The track currently loaded, by id
    pub(super) fn current_track_id(&self) -> Option<String> {
        let index = (*self.current_track_index.lock().unwrap())?;
        self.queue
            .lock()
            .unwrap()
            .get(index)
            .map(|track| track.id.clone())
    }

    /// Move a track to a new place in the queue
    ///
    /// Positions past the end land at the end rather than failing: a list being
    /// dragged about should not be able to produce an error the interface has to
    /// explain.
    pub fn move_track(&self, track_id: &str, to: usize) -> Result<()> {
        let moved = {
            let mut queue = self.queue.lock().unwrap();

            let from = queue
                .iter()
                .position(|track| track.id == track_id)
                .ok_or_else(|| anyhow::anyhow!("Track not found in queue"))?;

            let to = to.min(queue.len().saturating_sub(1));
            if from == to {
                return Ok(());
            }

            let track = queue
                .remove(from)
                .ok_or_else(|| anyhow::anyhow!("Track not found in queue"))?;
            queue.insert(to, track);

            (from, to)
        };

        let (from, to) = moved;
        self.shift_indices(from, to);

        Ok(())
    }

    /// Pause after this track, or stop pausing anywhere
    ///
    /// One instruction rather than a mode: it fires once and clears itself, so
    /// the panel never claims a break that has already happened is still coming.
    pub fn set_breakpoint(&self, track_id: Option<String>) {
        *self.breakpoint.lock().unwrap() = track_id;
    }

    pub fn breakpoint(&self) -> Option<String> {
        self.breakpoint.lock().unwrap().clone()
    }

    /// Whether the break falls after this track, taking it if it does
    pub(super) fn take_breakpoint_at(&self, track_id: &str) -> bool {
        let mut breakpoint = self.breakpoint.lock().unwrap();

        if breakpoint.as_deref() == Some(track_id) {
            *breakpoint = None;
            return true;
        }

        false
    }

    /// Drop the break if it was set after this track
    ///
    /// A break lives on the track it follows, so removing that track removes the
    /// instruction rather than sliding it onto whatever takes its place.
    pub(super) fn clear_breakpoint_if(&self, track_id: &str) {
        let mut breakpoint = self.breakpoint.lock().unwrap();

        if breakpoint.as_deref() == Some(track_id) {
            *breakpoint = None;
        }
    }

    /// Follow a track moving from one position to another
    fn shift_indices(&self, from: usize, to: usize) {
        let shift = |index: usize| -> usize {
            if index == from {
                to
            } else if from < index && index <= to {
                index - 1
            } else if to <= index && index < from {
                index + 1
            } else {
                index
            }
        };

        if let Some(current) = self.current_track_index.lock().unwrap().as_mut() {
            *current = shift(*current);
        }

        for entry in self.played_history.lock().unwrap().iter_mut() {
            *entry = shift(*entry);
        }
    }

    /// Close the gap a removed track leaves
    ///
    /// The track that was playing keeps playing — its decoder is already open —
    /// but the index it was at now names whatever moved up into its place, which
    /// is the track that plays next. That is the right answer for both.
    pub(super) fn forget_index(&self, removed: usize) {
        {
            let mut current = self.current_track_index.lock().unwrap();
            if let Some(index) = *current {
                if index > removed {
                    *current = Some(index - 1);
                }
            }
        }

        let mut history = self.played_history.lock().unwrap();
        history.retain(|entry| *entry != removed);
        for entry in history.iter_mut() {
            if *entry > removed {
                *entry -= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::file_player::player::QueuedTrack;
    use std::path::PathBuf;

    fn player_with(count: usize) -> AudioFilePlayer {
        let player = AudioFilePlayer::new(48_000, 2);

        for index in 0..count {
            player.enqueue(QueuedTrack {
                id: format!("track-{}", index),
                file_path: PathBuf::from(format!("/tmp/{}.wav", index)),
                title: None,
                artist: None,
                album: None,
                duration: None,
                file_size: 0,
                added_at: chrono::Utc::now(),
            });
        }

        player
    }

    fn ids(player: &AudioFilePlayer) -> Vec<String> {
        player
            .get_queue()
            .into_iter()
            .map(|track| track.id)
            .collect()
    }

    #[test]
    fn moving_a_track_later_reorders_the_queue() {
        let player = player_with(4);

        player.move_track("track-0", 2).unwrap();

        assert_eq!(
            ids(&player),
            vec!["track-1", "track-2", "track-0", "track-3"]
        );
    }

    #[test]
    fn moving_a_track_earlier_reorders_the_queue() {
        let player = player_with(4);

        player.move_track("track-3", 1).unwrap();

        assert_eq!(
            ids(&player),
            vec!["track-0", "track-3", "track-1", "track-2"]
        );
    }

    #[test]
    fn moving_past_the_end_lands_at_the_end() {
        let player = player_with(3);

        player.move_track("track-0", 99).unwrap();

        assert_eq!(ids(&player), vec!["track-1", "track-2", "track-0"]);
    }

    #[test]
    fn the_playing_track_is_followed_when_it_moves() {
        let player = player_with(4);
        *player.current_track_index.lock().unwrap() = Some(0);

        player.move_track("track-0", 2).unwrap();

        assert_eq!(player.current_track_id().as_deref(), Some("track-0"));
    }

    #[test]
    fn the_playing_track_is_followed_when_another_moves_past_it() {
        let player = player_with(4);
        *player.current_track_index.lock().unwrap() = Some(2);

        // Moving something from above it to below it shifts it up one
        player.move_track("track-0", 3).unwrap();

        assert_eq!(player.current_track_id().as_deref(), Some("track-2"));
    }

    #[test]
    fn removing_an_earlier_track_keeps_the_playing_one() {
        let player = player_with(4);
        *player.current_track_index.lock().unwrap() = Some(2);

        player.remove_track("track-0").unwrap();

        assert_eq!(player.current_track_id().as_deref(), Some("track-2"));
    }

    #[test]
    fn removing_a_later_track_keeps_the_playing_one() {
        let player = player_with(4);
        *player.current_track_index.lock().unwrap() = Some(1);

        player.remove_track("track-3").unwrap();

        assert_eq!(player.current_track_id().as_deref(), Some("track-1"));
    }

    #[test]
    fn removing_the_breakpoint_track_clears_the_break() {
        let player = player_with(3);
        player.set_breakpoint(Some("track-1".to_string()));

        player.remove_track("track-1").unwrap();

        assert_eq!(player.breakpoint(), None);
    }

    #[test]
    fn reordering_leaves_the_break_where_it_was_set() {
        let player = player_with(3);
        player.set_breakpoint(Some("track-2".to_string()));

        player.move_track("track-2", 0).unwrap();

        assert_eq!(player.breakpoint().as_deref(), Some("track-2"));
    }

    #[test]
    fn a_break_is_taken_once() {
        let player = player_with(2);
        player.set_breakpoint(Some("track-0".to_string()));

        assert!(player.take_breakpoint_at("track-0"));
        assert!(!player.take_breakpoint_at("track-0"));
    }

    #[test]
    fn history_follows_a_removed_track() {
        let player = player_with(5);
        *player.played_history.lock().unwrap() = vec![0, 3];
        *player.current_track_index.lock().unwrap() = Some(4);

        player.remove_track("track-1").unwrap();

        // Everything above the gap steps down one, and nothing points at the
        // track that went
        assert_eq!(*player.played_history.lock().unwrap(), vec![0, 2]);
        assert_eq!(player.current_track_id().as_deref(), Some("track-4"));
    }

    #[test]
    fn history_forgets_the_track_that_was_removed() {
        let player = player_with(4);
        *player.played_history.lock().unwrap() = vec![0, 1, 2];

        player.remove_track("track-1").unwrap();

        assert_eq!(*player.played_history.lock().unwrap(), vec![0, 1]);
    }
}
