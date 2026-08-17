// What is playing, as the transmitters need to see it.
//
// The interface already pushes the current track down when it changes, and
// Icecast forwards it on a second connection at that moment. Impulse cannot: it
// attaches metadata to a segment, so it needs to be able to ask what is playing
// at the moment a segment is built rather than being told at some other time.
//
// One slot rather than a channel, because there is no history worth keeping —
// a segment wants the current track, and a track that has been superseded is of
// no interest to the segment being built now.

use std::sync::{OnceLock, RwLock};

/// A track, as far as a listener is concerned
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
}

impl TrackMetadata {
    /// Whether this says anything worth sending
    pub fn is_empty(&self) -> bool {
        self.title.trim().is_empty() && self.artist.trim().is_empty()
    }
}

fn slot() -> &'static RwLock<Option<TrackMetadata>> {
    static SLOT: OnceLock<RwLock<Option<TrackMetadata>>> = OnceLock::new();
    SLOT.get_or_init(|| RwLock::new(None))
}

/// Record what is playing now
///
/// A reading with neither field clears the slot rather than storing two empty
/// strings, so "nothing is playing" reads as one state instead of two.
pub fn set(title: String, artist: String) {
    let track = TrackMetadata { title, artist };

    if let Ok(mut current) = slot().write() {
        *current = if track.is_empty() { None } else { Some(track) };
    }
}

/// What is playing, if anything has said
pub fn current() -> Option<TrackMetadata> {
    slot().read().ok().and_then(|track| track.clone())
}

/// Forget what was playing, which is what coming off air means
pub fn clear() {
    if let Ok(mut current) = slot().write() {
        *current = None;
    }
}

#[cfg(test)]
mod tests {
    use super::TrackMetadata;

    #[test]
    fn a_reading_with_nothing_in_it_is_empty() {
        assert!(TrackMetadata::default().is_empty());
        assert!(TrackMetadata {
            title: "  ".to_string(),
            artist: String::new(),
        }
        .is_empty());
    }

    /// A track with only one of the two fields is still worth sending — plenty
    /// of sources know the title and nothing else
    #[test]
    fn half_a_reading_is_still_a_reading() {
        assert!(!TrackMetadata {
            title: "Blue Monday".to_string(),
            artist: String::new(),
        }
        .is_empty());
    }
}
