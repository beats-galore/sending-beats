// What a file says about itself, read before it is ever played
//
// The queue shows a title, an artist and a length for every track, and it has to
// show them the moment a file is dropped in rather than once it reaches the
// decoder — a queue that only fills in as it plays is no use for deciding what
// to play next.
//
// Everything here is best-effort. A file with no tags is still a perfectly good
// file to queue, so nothing in this module fails: it returns what it found and
// leaves the rest empty for the interface to fall back on the file name.

use std::path::Path;
use std::time::Duration;

use symphonia::core::codecs::CODEC_TYPE_NULL;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardTagKey};
use symphonia::core::probe::Hint;

/// What a file says about itself
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<Duration>,
}

/// Read a file's tags and its length
///
/// Blocking: it opens the file and probes it. Call it off the async runtime.
pub fn read_metadata(path: &Path) -> TrackMetadata {
    let Ok(file) = std::fs::File::open(path) else {
        return TrackMetadata::default();
    };

    let stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }

    let probe = symphonia::default::get_probe().format(
        &hint,
        stream,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    );

    let Ok(mut probed) = probe else {
        return TrackMetadata::default();
    };

    // Two places to look. Containers that carry their own tags expose them on
    // the reader; a tag block the probe had to strip off the front to find the
    // stream at all — ID3v2 ahead of MP3 or FLAC — is only in the probe's log.
    let mut found = probed
        .format
        .metadata()
        .current()
        .map(tags_of)
        .unwrap_or_default();

    if found.is_empty() {
        if let Some(revision) = probed.metadata.get().as_ref().and_then(|log| log.current()) {
            found = tags_of(revision);
        }
    }

    TrackMetadata {
        title: found.title,
        artist: found.artist,
        album: found.album,
        duration: duration_of(probed.format.as_ref()),
    }
}

/// The three tags worth showing, out of whatever the file carries
#[derive(Debug, Default)]
struct FoundTags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
}

impl FoundTags {
    fn is_empty(&self) -> bool {
        self.title.is_none() && self.artist.is_none() && self.album.is_none()
    }
}

/// Pick the tags out of one revision
///
/// Read by standard key rather than by name: the same tag is spelled `TIT2` in
/// ID3, `TITLE` in Vorbis comments and `©nam` in MP4, and symphonia has already
/// done the work of mapping them onto one vocabulary.
fn tags_of(revision: &MetadataRevision) -> FoundTags {
    let mut found = FoundTags::default();

    for tag in revision.tags() {
        let value = tag.value.to_string();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }

        // The first of each wins. A file with two title tags has one title, and
        // the earlier revision is the one the format put first.
        match tag.std_key {
            Some(StandardTagKey::TrackTitle) if found.title.is_none() => {
                found.title = Some(value.to_string());
            }
            Some(StandardTagKey::Artist) if found.artist.is_none() => {
                found.artist = Some(value.to_string());
            }
            // Only when the track has no artist of its own, which is what a
            // compilation looks like: every track tagged to the album's artist.
            Some(StandardTagKey::AlbumArtist) if found.artist.is_none() => {
                found.artist = Some(value.to_string());
            }
            Some(StandardTagKey::Album) if found.album.is_none() => {
                found.album = Some(value.to_string());
            }
            _ => {}
        }
    }

    found
}

/// How long the file runs, when it says
///
/// Absent for a stream whose length is not written down anywhere — an MP3 with
/// no Xing header, most obviously. Counting it would mean decoding the whole
/// file, which is not worth doing to fill in a label.
fn duration_of(format: &dyn symphonia::core::formats::FormatReader) -> Option<Duration> {
    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)?;

    let frames = track.codec_params.n_frames?;

    if let Some(base) = track.codec_params.time_base {
        let time = base.calc_time(frames);
        return Some(Duration::from_secs_f64(time.seconds as f64 + time.frac));
    }

    let rate = track.codec_params.sample_rate?;
    Some(Duration::from_secs_f64(frames as f64 / rate as f64))
}
