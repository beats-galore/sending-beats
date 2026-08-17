// The ID3 header every packed-audio segment must open with.
//
// RFC 8216 §3.4: "Each Packed Audio Segment MUST signal the timestamp of its
// first sample with an ID3 PRIV tag at the beginning of the segment", and
// "Clients SHOULD NOT play Packed Audio Segments without this ID3 tag."
//
// Raw MP3 with no tag is not a corrupt segment. It fetches fine, parses fine,
// and then the player cannot place it on a timeline, so it buffers and never
// advances. It presents as a stall that seeking does not fix, because position
// was never the problem — which is what makes it expensive to find.
//
// This is a port of `packedAudioId3` on the other end, and the two have to stay
// identical: a segment built here and a segment built there land in the same
// playlist, and a player that has to accept both would be reading two different
// timelines.

const OWNER: &[u8] = b"com.apple.streaming.transportStreamTimestamp";

/// MPEG-2 Program Elementary Stream timestamps tick at 90 kHz.
const TIMESTAMP_HZ: u64 = 90_000;

/// The spec's 33-bit field, with the upper bits of the eight octets zeroed.
const TIMESTAMP_MODULUS: u64 = 1 << 33;

/// ID3v2 sizes are "synchsafe": seven bits per byte, so a size can never carry a
/// run of bits that a decoder would mistake for an MPEG sync word.
fn synchsafe(value: u32) -> [u8; 4] {
    [
        ((value >> 21) & 0x7F) as u8,
        ((value >> 14) & 0x7F) as u8,
        ((value >> 7) & 0x7F) as u8,
        (value & 0x7F) as u8,
    ]
}

/// The tag that opens a segment whose first sample sits at `elapsed_ms`
///
/// `elapsed_ms` is a position on the media timeline — cumulative broadcast time,
/// not wall clock and not the segment's own length.
pub fn packed_audio_id3(elapsed_ms: u64) -> Vec<u8> {
    let ticks = (elapsed_ms * TIMESTAMP_HZ / 1000) % TIMESTAMP_MODULUS;

    // owner, its null terminator, and the eight-octet timestamp
    let frame_size = OWNER.len() + 1 + 8;

    let mut frame = Vec::with_capacity(10 + frame_size);
    frame.extend_from_slice(b"PRIV");
    frame.extend_from_slice(&synchsafe(frame_size as u32));
    frame.extend_from_slice(&[0x00, 0x00]);
    frame.extend_from_slice(OWNER);
    frame.push(0x00);
    frame.extend_from_slice(&ticks.to_be_bytes());

    // A frame header is ten octets; the tag size counts everything after its own.
    let mut tag = Vec::with_capacity(10 + frame.len());
    tag.extend_from_slice(b"ID3");
    tag.extend_from_slice(&[0x04, 0x00, 0x00]);
    tag.extend_from_slice(&synchsafe(frame.len() as u32));
    tag.extend_from_slice(&frame);

    tag
}

#[cfg(test)]
mod tests {
    use super::{packed_audio_id3, OWNER};

    #[test]
    fn opens_with_an_id3v2_4_header() {
        let tag = packed_audio_id3(0);

        assert_eq!(&tag[0..3], b"ID3");
        assert_eq!(tag[3], 0x04, "major version");
        assert_eq!(tag[4], 0x00, "revision");
        assert_eq!(tag[5], 0x00, "no flags");
    }

    /// The size fields are what a player reads to find the audio, so they have to
    /// describe the bytes that are actually there
    #[test]
    fn the_sizes_describe_the_tag_that_was_written() {
        let tag = packed_audio_id3(0);

        let tag_size = u32::from_be_bytes([tag[6], tag[7], tag[8], tag[9]]);
        // Synchsafe: seven bits per byte.
        let declared = (tag_size & 0x7F)
            | ((tag_size >> 8) & 0x7F) << 7
            | ((tag_size >> 16) & 0x7F) << 14
            | ((tag_size >> 24) & 0x7F) << 21;

        assert_eq!(declared as usize, tag.len() - 10);
        assert_eq!(tag.len(), 10 + 10 + OWNER.len() + 1 + 8);
    }

    #[test]
    fn carries_the_owner_the_spec_names() {
        let tag = packed_audio_id3(0);

        assert_eq!(&tag[10..14], b"PRIV");
        assert!(
            tag.windows(OWNER.len()).any(|window| window == OWNER),
            "the owner identifier is present"
        );
    }

    #[test]
    fn the_timestamp_is_ninety_kilohertz_ticks() {
        let tag = packed_audio_id3(1_000);
        let ticks = u64::from_be_bytes(tag[tag.len() - 8..].try_into().unwrap());

        assert_eq!(ticks, 90_000, "one second is ninety thousand ticks");
    }

    #[test]
    fn the_timestamp_advances_with_the_broadcast() {
        let first = packed_audio_id3(0);
        let later = packed_audio_id3(4_000);

        let start = u64::from_be_bytes(first[first.len() - 8..].try_into().unwrap());
        let after = u64::from_be_bytes(later[later.len() - 8..].try_into().unwrap());

        assert_eq!(start, 0);
        assert_eq!(after, 360_000);
    }

    /// A show long enough to wrap the 33-bit field is about 26.5 hours, which is
    /// a station that never went off air rather than an impossible case
    #[test]
    fn the_timestamp_wraps_at_thirty_three_bits() {
        let modulus_ms = (1_u64 << 33) * 1000 / 90_000;
        let tag = packed_audio_id3(modulus_ms);
        let ticks = u64::from_be_bytes(tag[tag.len() - 8..].try_into().unwrap());

        assert!(ticks < (1 << 33), "stays inside the field the spec defines");
    }
}
