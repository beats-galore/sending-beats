// Cutting the encoder's output into segments that can each be sent on their own.
//
// The encoder hands over whatever bytes it happens to have finished with, which
// is neither aligned to frames nor to any useful length. This holds them until
// there are whole frames, keeps whole frames until there are enough of them to
// be worth a request, and then hands over a segment with a measured duration and
// the ID3 tag a player needs to place it on a timeline.
//
// Duration is counted in samples rather than milliseconds. A frame at 44.1 kHz
// is 26.12 ms, and adding that up as a rounded integer 900 times an hour walks
// the playlist away from the audio.

use super::mp3_frames::{scan, Scan};
use super::packed_audio_id3::packed_audio_id3;

/// One segment, ready to be sent
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// The ID3 tag followed by whole MP3 frames
    pub body: Vec<u8>,
    /// Measured from the frames actually inside it, not the target
    pub duration_ms: u64,
    /// Where its first sample sits on the media timeline
    pub elapsed_ms: u64,
}

/// Accumulates encoded audio and cuts it on frame boundaries
#[derive(Debug)]
pub struct Mp3Segmenter {
    target_ms: u64,
    /// Bytes that have not yet been resolved into whole frames
    pending: Vec<u8>,
    /// Whole frames belonging to the segment being assembled
    frames: Vec<u8>,
    /// Samples per channel in `frames`
    frame_samples: u64,
    /// Samples per channel already sent, which is the media timeline position
    sent_samples: u64,
    /// Learned from the first frame; every frame after it agrees
    sample_rate: Option<u32>,
}

impl Mp3Segmenter {
    pub fn new(target_ms: u64) -> Self {
        Self {
            target_ms: target_ms.max(1),
            pending: Vec::new(),
            frames: Vec::new(),
            frame_samples: 0,
            sent_samples: 0,
            sample_rate: None,
        }
    }

    /// Take whatever the encoder produced
    pub fn push(&mut self, encoded: &[u8]) {
        if encoded.is_empty() {
            return;
        }

        self.pending.extend_from_slice(encoded);

        loop {
            match scan(&self.pending) {
                Scan::Frame { at, header } => {
                    let end = at + header.length;
                    self.frames.extend_from_slice(&self.pending[at..end]);
                    self.frame_samples += header.samples as u64;
                    self.sample_rate.get_or_insert(header.sample_rate);
                    self.pending.drain(..end);
                }
                // A header is here but its frame is not all in yet. Anything in
                // front of it is not audio, so it goes.
                Scan::Partial { at } => {
                    self.pending.drain(..at);
                    break;
                }
                Scan::Nothing { consumed } => {
                    self.pending.drain(..consumed);
                    break;
                }
            }
        }
    }

    /// A segment, once enough frames have been collected for one
    pub fn take(&mut self) -> Option<Segment> {
        let rate = self.sample_rate?;

        if self.frame_samples < self.target_samples(rate) {
            return None;
        }

        Some(self.cut(rate))
    }

    /// Whatever is left, however short
    ///
    /// Called when a broadcast ends. A two second final segment is a two second
    /// final segment; dropping it loses the end of the show.
    pub fn flush(&mut self) -> Option<Segment> {
        let rate = self.sample_rate?;

        if self.frames.is_empty() {
            return None;
        }

        Some(self.cut(rate))
    }

    fn target_samples(&self, rate: u32) -> u64 {
        self.target_ms * rate as u64 / 1000
    }

    fn cut(&mut self, rate: u32) -> Segment {
        let frames = std::mem::take(&mut self.frames);
        let samples = std::mem::replace(&mut self.frame_samples, 0);

        let elapsed_ms = self.sent_samples * 1000 / rate as u64;
        let duration_ms = samples * 1000 / rate as u64;
        self.sent_samples += samples;

        let mut body = packed_audio_id3(elapsed_ms);
        body.extend_from_slice(&frames);

        Segment {
            body,
            // A segment the other end is told lasts zero seconds is one it
            // rejects, and a sub-millisecond segment is not audio anyone needs.
            duration_ms: duration_ms.max(1),
            elapsed_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Mp3Segmenter, Segment};

    /// 128 kbps, 48 kHz, MPEG1 Layer III — 384 bytes, 1152 samples, 24 ms
    const FRAME_BYTES: usize = 384;
    const FRAME_MS: u64 = 24;

    fn frame() -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xFB, 0x94, 0x00];
        bytes.resize(FRAME_BYTES, 0x5A);
        bytes
    }

    fn frames(count: usize) -> Vec<u8> {
        (0..count).flat_map(|_| frame()).collect()
    }

    /// Strip the ID3 tag a segment opens with, leaving the audio
    fn audio_of(segment: &Segment) -> &[u8] {
        let tag_size = u32::from_be_bytes([
            segment.body[6],
            segment.body[7],
            segment.body[8],
            segment.body[9],
        ]);
        let declared = (tag_size & 0x7F)
            | ((tag_size >> 8) & 0x7F) << 7
            | ((tag_size >> 16) & 0x7F) << 14
            | ((tag_size >> 24) & 0x7F) << 21;

        &segment.body[10 + declared as usize..]
    }

    #[test]
    fn nothing_comes_out_before_the_target_is_reached() {
        let mut segmenter = Mp3Segmenter::new(4_000);
        segmenter.push(&frames(10));

        assert!(segmenter.take().is_none(), "240 ms is not four seconds");
    }

    #[test]
    fn a_segment_appears_once_there_is_enough_audio() {
        let mut segmenter = Mp3Segmenter::new(4_000);
        // 167 frames is 4008 ms, the first count over four seconds.
        segmenter.push(&frames(167));

        let segment = segmenter.take().expect("a segment");

        assert_eq!(segment.elapsed_ms, 0, "the first one starts the timeline");
        assert_eq!(segment.duration_ms, 167 * FRAME_MS);
        assert_eq!(audio_of(&segment).len(), 167 * FRAME_BYTES);
    }

    /// The duration reported has to be the frames that are there, because the
    /// playlist on the other end is built from it
    #[test]
    fn the_duration_is_measured_rather_than_assumed() {
        let mut segmenter = Mp3Segmenter::new(1_000);
        segmenter.push(&frames(42));

        assert_eq!(segmenter.take().unwrap().duration_ms, 42 * FRAME_MS);
    }

    #[test]
    fn the_timeline_advances_by_what_was_sent() {
        let mut segmenter = Mp3Segmenter::new(1_000);

        segmenter.push(&frames(42));
        let first = segmenter.take().expect("a segment");

        segmenter.push(&frames(42));
        let second = segmenter.take().expect("another segment");

        assert_eq!(first.elapsed_ms, 0);
        assert_eq!(second.elapsed_ms, first.duration_ms);
    }

    /// The encoder hands over arbitrary chunk sizes, so frames arrive in pieces
    #[test]
    fn frames_split_across_pushes_are_reassembled() {
        let mut segmenter = Mp3Segmenter::new(1_000);
        let audio = frames(42);

        for chunk in audio.chunks(97) {
            segmenter.push(chunk);
        }

        let segment = segmenter.take().expect("a segment");
        assert_eq!(audio_of(&segment).len(), 42 * FRAME_BYTES);
        assert_eq!(audio_of(&segment), &audio[..]);
    }

    #[test]
    fn every_segment_opens_with_the_id3_tag() {
        let mut segmenter = Mp3Segmenter::new(1_000);
        segmenter.push(&frames(42));

        let segment = segmenter.take().expect("a segment");
        assert_eq!(&segment.body[0..3], b"ID3");
        assert_eq!(audio_of(&segment)[0], 0xFF, "the audio follows the tag");
    }

    #[test]
    fn the_end_of_a_show_is_not_dropped() {
        let mut segmenter = Mp3Segmenter::new(4_000);
        segmenter.push(&frames(10));

        assert!(segmenter.take().is_none());

        let tail = segmenter.flush().expect("the short final segment");
        assert_eq!(tail.duration_ms, 10 * FRAME_MS);
    }

    #[test]
    fn flushing_an_empty_segmenter_produces_nothing() {
        let mut segmenter = Mp3Segmenter::new(4_000);
        assert!(segmenter.flush().is_none());

        segmenter.push(&frames(4));
        assert!(segmenter.flush().is_some());
        assert!(segmenter.flush().is_none(), "and not twice");
    }

    /// A resync after rubbish on the wire should find the audio rather than
    /// treat the whole buffer as lost
    #[test]
    fn leading_rubbish_is_skipped() {
        let mut segmenter = Mp3Segmenter::new(1_000);

        let mut audio = vec![0x00, 0x11, 0x22];
        audio.extend_from_slice(&frames(42));
        segmenter.push(&audio);

        assert_eq!(audio_of(&segmenter.take().unwrap()).len(), 42 * FRAME_BYTES);
    }
}
