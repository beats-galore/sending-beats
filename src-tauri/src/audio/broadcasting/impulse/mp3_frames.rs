// Finding frame boundaries in an MP3 stream, and measuring what is inside them.
//
// Segmenting means cutting the encoder's output somewhere a decoder can start
// again, and an MPEG audio frame declares everything needed to find that place
// in its first four bytes: how long it is and how many samples it carries. No
// codec is involved — this reads headers and never touches the audio.
//
// The measured duration matters more than it looks. `#EXTINF` on the other end
// is the summed duration of the frames actually in a segment, not the nominal
// four seconds, and a playlist whose durations disagree with its audio drifts a
// listener out of sync a little at a time.

/// What a frame header says about its frame
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Whole frame length in bytes, header included
    pub length: usize,
    /// Samples per channel this frame carries
    pub samples: u32,
    pub sample_rate: u32,
}

/// MPEG version, from bits 20-19
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Version {
    Mpeg1,
    Mpeg2,
    Mpeg25,
}

/// Layer, from bits 18-17
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    One,
    Two,
    Three,
}

const MPEG1_LAYER1: [u32; 15] = [
    0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
];
const MPEG1_LAYER2: [u32; 15] = [
    0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
];
const MPEG1_LAYER3: [u32; 15] = [
    0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
];
const MPEG2_LAYER1: [u32; 15] = [
    0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
];
const MPEG2_LAYER23: [u32; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];

const MPEG1_RATES: [u32; 3] = [44_100, 48_000, 32_000];
const MPEG2_RATES: [u32; 3] = [22_050, 24_000, 16_000];
const MPEG25_RATES: [u32; 3] = [11_025, 12_000, 8_000];

/// Read the four header bytes at the front of `bytes`
///
/// `None` means these four bytes are not a frame header — either they are audio
/// that happens to be here, or the stream needs resynchronising. Every reserved
/// and "free" encoding is rejected rather than guessed at, because a wrong
/// length puts the next read in the middle of a frame and the error compounds.
pub fn parse_header(bytes: &[u8]) -> Option<FrameHeader> {
    if bytes.len() < 4 {
        return None;
    }

    // Eleven sync bits, all set.
    if bytes[0] != 0xFF || bytes[1] & 0xE0 != 0xE0 {
        return None;
    }

    let version = match (bytes[1] >> 3) & 0b11 {
        0b00 => Version::Mpeg25,
        0b10 => Version::Mpeg2,
        0b11 => Version::Mpeg1,
        _ => return None,
    };

    let layer = match (bytes[1] >> 1) & 0b11 {
        0b01 => Layer::Three,
        0b10 => Layer::Two,
        0b11 => Layer::One,
        _ => return None,
    };

    let bitrate_index = (bytes[2] >> 4) as usize;
    // 0 is "free format", where the length is not in the header at all, and 15
    // is reserved. Neither can be cut on.
    if bitrate_index == 0 || bitrate_index >= 15 {
        return None;
    }

    let rate_index = ((bytes[2] >> 2) & 0b11) as usize;
    if rate_index >= 3 {
        return None;
    }

    let kbps = match (version, layer) {
        (Version::Mpeg1, Layer::One) => MPEG1_LAYER1[bitrate_index],
        (Version::Mpeg1, Layer::Two) => MPEG1_LAYER2[bitrate_index],
        (Version::Mpeg1, Layer::Three) => MPEG1_LAYER3[bitrate_index],
        (_, Layer::One) => MPEG2_LAYER1[bitrate_index],
        (_, _) => MPEG2_LAYER23[bitrate_index],
    };

    let sample_rate = match version {
        Version::Mpeg1 => MPEG1_RATES[rate_index],
        Version::Mpeg2 => MPEG2_RATES[rate_index],
        Version::Mpeg25 => MPEG25_RATES[rate_index],
    };

    let padding = ((bytes[2] >> 1) & 1) as usize;
    let samples = samples_per_frame(version, layer);

    // Layer I counts its padding in four-byte slots; the other two count bytes.
    let length = match layer {
        Layer::One => (12 * kbps as usize * 1000 / sample_rate as usize + padding) * 4,
        _ => (samples as usize / 8) * kbps as usize * 1000 / sample_rate as usize + padding,
    };

    if length <= 4 {
        return None;
    }

    Some(FrameHeader {
        length,
        samples,
        sample_rate,
    })
}

fn samples_per_frame(version: Version, layer: Layer) -> u32 {
    match (version, layer) {
        (_, Layer::One) => 384,
        (Version::Mpeg1, _) => 1152,
        // Layer III halves at the lower versions; Layer II does not.
        (_, Layer::Two) => 1152,
        (_, Layer::Three) => 576,
    }
}

/// What the next read of a buffer found
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scan {
    /// A whole frame, starting at `at`
    Frame { at: usize, header: FrameHeader },
    /// A header at `at`, but the rest of its frame has not arrived yet
    Partial { at: usize },
    /// Nothing usable. `consumed` bytes can be discarded; the remainder is kept
    /// because a header could be straddling the end of the buffer.
    Nothing { consumed: usize },
}

/// Find the next whole frame in `bytes`
pub fn scan(bytes: &[u8]) -> Scan {
    let mut at = 0;

    while at + 4 <= bytes.len() {
        if let Some(header) = parse_header(&bytes[at..]) {
            return if at + header.length <= bytes.len() {
                Scan::Frame { at, header }
            } else {
                Scan::Partial { at }
            };
        }

        at += 1;
    }

    // Three bytes are held back: the first byte of a header can be the last byte
    // of what has arrived so far.
    Scan::Nothing {
        consumed: bytes.len().saturating_sub(3),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_header, scan, Scan};

    /// 128 kbps, 44.1 kHz, MPEG1 Layer III, no padding
    fn header_128k_44100() -> [u8; 4] {
        [0xFF, 0xFB, 0x90, 0x00]
    }

    #[test]
    fn reads_a_layer_three_frame() {
        let header = parse_header(&header_128k_44100()).expect("a valid header");

        assert_eq!(header.sample_rate, 44_100);
        assert_eq!(header.samples, 1152);
        // 1152 / 8 * 128000 / 44100 = 417
        assert_eq!(header.length, 417);
    }

    #[test]
    fn padding_adds_a_byte() {
        let mut bytes = header_128k_44100();
        bytes[2] |= 0b10;

        assert_eq!(parse_header(&bytes).unwrap().length, 418);
    }

    /// 48 kHz divides evenly, which is the case the studio actually runs at
    #[test]
    fn reads_a_48k_frame() {
        let header = parse_header(&[0xFF, 0xFB, 0x94, 0x00]).expect("a valid header");

        assert_eq!(header.sample_rate, 48_000);
        // 1152 / 8 * 128000 / 48000 = 384
        assert_eq!(header.length, 384);
    }

    #[test]
    fn a_free_format_frame_is_refused() {
        // Bitrate index 0 carries no length, so there is nothing to cut on.
        assert!(parse_header(&[0xFF, 0xFB, 0x00, 0x00]).is_none());
    }

    #[test]
    fn a_reserved_sample_rate_is_refused() {
        assert!(parse_header(&[0xFF, 0xFB, 0x9C, 0x00]).is_none());
    }

    #[test]
    fn nonsense_is_not_a_header() {
        assert!(parse_header(&[0x00, 0x00, 0x00, 0x00]).is_none());
        assert!(parse_header(&[0xFF]).is_none());
    }

    #[test]
    fn scanning_finds_a_whole_frame_at_the_front() {
        let mut bytes = header_128k_44100().to_vec();
        bytes.resize(417, 0);

        assert_eq!(
            scan(&bytes),
            Scan::Frame {
                at: 0,
                header: parse_header(&header_128k_44100()).unwrap()
            }
        );
    }

    /// The encoder hands over arbitrary chunks, so half a frame is the ordinary
    /// case rather than an error
    #[test]
    fn a_half_arrived_frame_waits() {
        let mut bytes = header_128k_44100().to_vec();
        bytes.resize(200, 0);

        assert_eq!(scan(&bytes), Scan::Partial { at: 0 });
    }

    #[test]
    fn scanning_skips_leading_rubbish() {
        let mut bytes = vec![0x11, 0x22, 0x33];
        bytes.extend_from_slice(&header_128k_44100());
        bytes.resize(3 + 417, 0);

        assert!(matches!(scan(&bytes), Scan::Frame { at: 3, .. }));
    }

    /// Nothing usable still keeps the tail, because a sync word can straddle
    #[test]
    fn a_buffer_with_no_frame_keeps_its_tail() {
        assert_eq!(
            scan(&[0x00, 0x00, 0x00, 0x00, 0x00]),
            Scan::Nothing { consumed: 2 }
        );
    }
}
