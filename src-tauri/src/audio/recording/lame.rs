// A binding to LAME that can finish a file.
//
// The `lame` crate binds five setters and `lame_encode_buffer`, and nothing
// else. That is enough to write most of an MP3 and not enough to end one:
//
// - `lame_encode_flush` is what emits the samples still inside the encoder.
//   Without it the last frames of every recording are simply lost.
// - `lame_get_lametag_frame` is the Xing/LAME header. Without it a player has
//   no seek table and no frame count, so scrubbing does not work and the
//   reported duration is a guess from the bitrate.
// - Its `encode` panics unless both channel slices are the same length, which
//   makes a mono recording a crash rather than a file.
//
// The crate also keeps its `lame_t` private, so none of this can be added from
// outside it. libmp3lame is already linked for the crate's own use, so these
// declare the rest of the calls we need and wrap them safely.

use anyhow::Result;
use std::os::raw::{c_int, c_void};

type LamePtr = *mut c_void;

#[link(name = "mp3lame")]
extern "C" {
    fn lame_init() -> LamePtr;
    fn lame_close(ptr: LamePtr) -> c_int;
    fn lame_set_in_samplerate(ptr: LamePtr, rate: c_int) -> c_int;
    fn lame_set_num_channels(ptr: LamePtr, channels: c_int) -> c_int;
    fn lame_set_brate(ptr: LamePtr, kbps: c_int) -> c_int;
    fn lame_set_quality(ptr: LamePtr, quality: c_int) -> c_int;
    fn lame_set_bWriteVbrTag(ptr: LamePtr, write: c_int) -> c_int;
    fn lame_set_disable_reservoir(ptr: LamePtr, disable: c_int) -> c_int;
    fn lame_init_params(ptr: LamePtr) -> c_int;

    fn lame_encode_buffer_interleaved_ieee_float(
        ptr: LamePtr,
        pcm: *const f32,
        samples_per_channel: c_int,
        mp3buf: *mut u8,
        mp3buf_size: c_int,
    ) -> c_int;

    fn lame_encode_buffer_ieee_float(
        ptr: LamePtr,
        pcm_left: *const f32,
        pcm_right: *const f32,
        samples_per_channel: c_int,
        mp3buf: *mut u8,
        mp3buf_size: c_int,
    ) -> c_int;

    fn lame_encode_flush(ptr: LamePtr, mp3buf: *mut u8, mp3buf_size: c_int) -> c_int;

    fn lame_get_lametag_frame(ptr: LamePtr, buffer: *mut u8, size: usize) -> usize;
}

/// Room LAME asks for, as its own documentation states it
///
/// 1.25 samples per input sample plus 7200 for a worst-case frame. Generous on
/// purpose: an undersized buffer is an encode error rather than a short write.
fn buffer_for(samples_per_channel: usize) -> usize {
    (samples_per_channel as f64 * 1.25) as usize + 7200
}

/// A LAME encoder that can be started, fed, and properly finished
pub struct Lame {
    ptr: LamePtr,
    channels: u16,
}

// SAFETY: the pointer is owned by this struct and never shared. The recording
// writer is the only thing that touches an encoder, one task at a time.
unsafe impl Send for Lame {}

/// Whether the encoder is producing one file or an endless stream cut up later
///
/// A file wants the seek table and the bit reservoir: it is read from the start
/// and every frame is reachable. A stream that will be cut into segments wants
/// neither — the seek table describes a length nothing has yet, and a frame that
/// borrows bits from the frame before it stops decoding cleanly the moment those
/// two land in different segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LameOutput {
    WholeFile,
    Segmented,
}

impl Lame {
    pub fn new(sample_rate: u32, channels: u16, kilobitrate: u32) -> Result<Self> {
        Self::build(sample_rate, channels, kilobitrate, LameOutput::WholeFile)
    }

    /// An encoder whose frames can be split apart at any frame boundary
    pub fn for_segments(sample_rate: u32, channels: u16, kilobitrate: u32) -> Result<Self> {
        Self::build(sample_rate, channels, kilobitrate, LameOutput::Segmented)
    }

    fn build(
        sample_rate: u32,
        channels: u16,
        kilobitrate: u32,
        output: LameOutput,
    ) -> Result<Self> {
        let ptr = unsafe { lame_init() };
        if ptr.is_null() {
            return Err(anyhow::anyhow!("Could not create a LAME encoder"));
        }

        let lame = Self { ptr, channels };

        // Checked one at a time: LAME reports a bad setting by return code and
        // then encodes happily at some other value, which is worse than saying
        // so up front.
        lame.set("sample rate", unsafe {
            lame_set_in_samplerate(ptr, sample_rate as c_int)
        })?;
        lame.set("channels", unsafe {
            lame_set_num_channels(ptr, channels as c_int)
        })?;
        lame.set("bitrate", unsafe {
            lame_set_brate(ptr, kilobitrate as c_int)
        })?;
        // 2 is LAME's own "high quality, still fast" setting.
        lame.set("quality", unsafe { lame_set_quality(ptr, 2) })?;
        // Reserves the frame at the front of the file that `lametag_frame`
        // fills in at the end. Without the reservation there is nowhere to put
        // the seek table.
        let whole_file = output == LameOutput::WholeFile;
        lame.set("vbr tag", unsafe {
            lame_set_bWriteVbrTag(ptr, whole_file as c_int)
        })?;
        // Layer III lets a frame keep some of its bits in the frames before it.
        // That is free quality in a file and a defect in a segment: the first
        // frames of every segment would be reaching for data that went out in
        // the previous request, and a player joining mid-stream would decode
        // them wrong.
        lame.set("bit reservoir", unsafe {
            lame_set_disable_reservoir(ptr, !whole_file as c_int)
        })?;

        lame.set("parameters", unsafe { lame_init_params(ptr) })?;

        Ok(lame)
    }

    fn set(&self, what: &str, status: c_int) -> Result<()> {
        if status < 0 {
            return Err(anyhow::anyhow!("LAME refused the {}: {}", what, status));
        }
        Ok(())
    }

    /// Encode interleaved float samples
    ///
    /// Mono is handed the same buffer for both sides rather than an empty one:
    /// LAME ignores the right channel when it was told there is one channel,
    /// and an empty slice is what made the previous binding panic.
    pub fn encode(&mut self, interleaved: &[f32]) -> Result<Vec<u8>> {
        if interleaved.is_empty() {
            return Ok(Vec::new());
        }

        let per_channel = interleaved.len() / self.channels.max(1) as usize;
        let mut out = vec![0u8; buffer_for(per_channel)];

        let written = if self.channels == 1 {
            unsafe {
                lame_encode_buffer_ieee_float(
                    self.ptr,
                    interleaved.as_ptr(),
                    interleaved.as_ptr(),
                    per_channel as c_int,
                    out.as_mut_ptr(),
                    out.len() as c_int,
                )
            }
        } else {
            unsafe {
                lame_encode_buffer_interleaved_ieee_float(
                    self.ptr,
                    interleaved.as_ptr(),
                    per_channel as c_int,
                    out.as_mut_ptr(),
                    out.len() as c_int,
                )
            }
        };

        if written < 0 {
            return Err(anyhow::anyhow!("LAME encoding failed: {}", written));
        }

        out.truncate(written as usize);
        Ok(out)
    }

    /// Everything still inside the encoder, which ends the file
    pub fn flush(&mut self) -> Result<Vec<u8>> {
        let mut out = vec![0u8; buffer_for(0)];

        let written = unsafe { lame_encode_flush(self.ptr, out.as_mut_ptr(), out.len() as c_int) };

        if written < 0 {
            return Err(anyhow::anyhow!("LAME flush failed: {}", written));
        }

        out.truncate(written as usize);
        Ok(out)
    }

    /// The Xing/LAME header, to be written over the frame reserved at the front
    ///
    /// Only meaningful after `flush`: it carries the frame count and the seek
    /// table, and neither is known until the encoder has finished.
    pub fn tag_frame(&self) -> Vec<u8> {
        // Asked with an empty buffer first, which is how LAME reports the size
        // it needs rather than making the caller guess.
        let needed = unsafe { lame_get_lametag_frame(self.ptr, std::ptr::null_mut(), 0) };
        if needed == 0 {
            return Vec::new();
        }

        let mut out = vec![0u8; needed];
        let written = unsafe { lame_get_lametag_frame(self.ptr, out.as_mut_ptr(), out.len()) };

        out.truncate(written);
        out
    }
}

impl Drop for Lame {
    fn drop(&mut self) {
        unsafe { lame_close(self.ptr) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stereo_encoder_flushes_and_produces_a_tag() {
        let mut lame = Lame::new(48_000, 2, 192).expect("LAME available");

        // A second of quiet stereo. Silence still encodes to frames.
        let samples = vec![0.0_f32; 48_000 * 2];
        let encoded = lame.encode(&samples).expect("encodes");

        let tail = lame.flush().expect("flushes");
        let tag = lame.tag_frame();

        assert!(!encoded.is_empty(), "a second of audio produces frames");
        assert!(!tail.is_empty(), "the flush produces the final frame");
        assert!(!tag.is_empty(), "a tag frame is produced for the header");

        // An MPEG audio frame: eleven sync bits, so 0xFF then the top three
        // bits of the next byte.
        assert_eq!(tag[0], 0xFF, "the tag frame starts with the MPEG sync");
        assert_eq!(
            tag[1] & 0xE0,
            0xE0,
            "the sync word runs into the second byte"
        );

        // The tag itself sits after the frame header and side info. LAME writes
        // "Info" for constant bitrate and "Xing" for variable.
        let marker = tag
            .windows(4)
            .position(|window| window == b"Info" || window == b"Xing");
        assert!(
            marker.is_some(),
            "the frame carries a Xing/Info tag: {:02x?}",
            &tag[..16]
        );
    }

    /// The previous binding panicked here: it required both channel slices to
    /// be the same length, and mono had nothing to put in the second.
    #[test]
    fn a_mono_encoder_does_not_panic() {
        let mut lame = Lame::new(48_000, 1, 128).expect("LAME available");

        let samples = vec![0.1_f32; 48_000];
        let encoded = lame.encode(&samples).expect("encodes");
        let tail = lame.flush().expect("flushes");

        assert!(!encoded.is_empty() || !tail.is_empty());
    }

    #[test]
    fn encoding_nothing_is_not_an_error() {
        let mut lame = Lame::new(48_000, 2, 192).expect("LAME available");
        assert!(lame.encode(&[]).expect("no error").is_empty());
    }
}
