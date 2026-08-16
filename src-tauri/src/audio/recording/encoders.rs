// Audio encoding implementations for different formats
//
// This module provides audio encoders for WAV, MP3, and FLAC formats.
// Each encoder implements the AudioEncoder trait for consistent interface
// while handling format-specific encoding requirements.

use anyhow::Result;
use tracing::{error, info, warn};

use super::types::{FlacSettings, Mp3Settings, RecordingConfig};

/// Common interface for audio encoders
pub trait AudioEncoder: Send {
    /// Initialize the encoder with configuration
    fn initialize(&mut self, config: &RecordingConfig) -> Result<()>;

    /// Encode audio samples and return encoded data
    fn encode(&mut self, samples: &[f32]) -> Result<Vec<u8>>;

    /// Finalize encoding and return any remaining data
    fn finalize(&mut self) -> Result<Vec<u8>>;

    /// Get the file extension for this encoder
    fn file_extension(&self) -> &'static str;

    /// Get encoder-specific metadata
    fn get_metadata(&self) -> EncoderMetadata;

    /// Byte patches to apply to the closed file, as `(offset, little-endian bytes)`.
    ///
    /// Container formats that record total lengths in a fixed-size header cannot know
    /// those lengths until the stream ends, so the writer seeks back and applies these
    /// once the file is complete. Streaming formats return nothing.
    fn finalize_patches(&self) -> Vec<(u64, Vec<u8>)> {
        Vec::new()
    }
}

/// Metadata about an encoder's current state
#[derive(Debug, Clone)]
pub struct EncoderMetadata {
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: u16,
    pub samples_encoded: u64,
    pub bytes_written: u64,
    pub encoder_name: Option<String>,
}

impl Default for EncoderMetadata {
    fn default() -> Self {
        Self {
            sample_rate: 0,
            channels: 0,
            bit_depth: 0,
            samples_encoded: 0,
            bytes_written: 0,
            encoder_name: None,
        }
    }
}

/// Canonical WAV header: RIFF descriptor (12) + 16-byte fmt chunk (24) + data chunk header (8)
const WAV_HEADER_LEN: u32 = 44;
/// Offset of the RIFF chunk size field, which covers everything after it
const WAV_RIFF_SIZE_OFFSET: u64 = 4;
/// Offset of the data chunk size field, which covers the samples alone
const WAV_DATA_SIZE_OFFSET: u64 = 40;

/// WAV format encoder - simple uncompressed PCM
pub struct WavEncoder {
    metadata: EncoderMetadata,
    header_written: bool,
}

impl WavEncoder {
    /// Create a new WAV encoder
    pub fn new() -> Self {
        Self {
            metadata: EncoderMetadata::default(),
            header_written: false,
        }
    }

    /// Generate WAV header for the current configuration
    fn generate_wav_header(&self) -> Vec<u8> {
        let sample_rate = self.metadata.sample_rate;
        let channels = self.metadata.channels;
        let bit_depth = self.metadata.bit_depth;

        let byte_rate = sample_rate * channels as u32 * (bit_depth as u32 / 8);
        let block_align = channels * (bit_depth / 8);

        let mut header = Vec::with_capacity(44);

        // RIFF header - sizes are patched in by finalize_patches once the stream ends
        header.extend_from_slice(b"RIFF");
        header.extend_from_slice(&[0, 0, 0, 0]); // File size placeholder
        header.extend_from_slice(b"WAVE");

        // fmt chunk
        header.extend_from_slice(b"fmt ");
        header.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        header.extend_from_slice(&1u16.to_le_bytes()); // PCM format
        header.extend_from_slice(&channels.to_le_bytes());
        header.extend_from_slice(&sample_rate.to_le_bytes());
        header.extend_from_slice(&byte_rate.to_le_bytes());
        header.extend_from_slice(&block_align.to_le_bytes());
        header.extend_from_slice(&bit_depth.to_le_bytes());

        // data chunk header
        header.extend_from_slice(b"data");
        header.extend_from_slice(&[0, 0, 0, 0]); // Data size placeholder

        header
    }

    /// Convert f32 samples to the target bit depth
    fn convert_samples(&self, samples: &[f32]) -> Vec<u8> {
        match self.metadata.bit_depth {
            16 => {
                let mut output = Vec::with_capacity(samples.len() * 2);
                for &sample in samples {
                    let sample_i16 = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                    output.extend_from_slice(&sample_i16.to_le_bytes());
                }
                output
            }
            24 => {
                let mut output = Vec::with_capacity(samples.len() * 3);
                for &sample in samples {
                    let sample_i32 = (sample.clamp(-1.0, 1.0) * 8388607.0) as i32;
                    output.push((sample_i32 & 0xFF) as u8);
                    output.push(((sample_i32 >> 8) & 0xFF) as u8);
                    output.push(((sample_i32 >> 16) & 0xFF) as u8);
                }
                output
            }
            32 => {
                let mut output = Vec::with_capacity(samples.len() * 4);
                for &sample in samples {
                    // Scaled in f64 because i32::MAX has no exact f32 representation
                    let sample_i32 = (f64::from(sample.clamp(-1.0, 1.0)) * 2147483647.0) as i32;
                    output.extend_from_slice(&sample_i32.to_le_bytes());
                }
                output
            }
            _ => {
                error!("Unsupported bit depth: {}", self.metadata.bit_depth);
                Vec::new()
            }
        }
    }
}

impl AudioEncoder for WavEncoder {
    fn initialize(&mut self, config: &RecordingConfig) -> Result<()> {
        self.metadata = EncoderMetadata {
            sample_rate: config.sample_rate,
            channels: config.channels,
            bit_depth: config.bit_depth,
            samples_encoded: 0,
            bytes_written: 0,
            encoder_name: Some("WAV PCM".to_string()),
        };
        self.header_written = false;

        info!(
            "WAV encoder initialized: {}Hz, {} channels, {} bit",
            config.sample_rate, config.channels, config.bit_depth
        );
        Ok(())
    }

    fn encode(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }

        let mut output = Vec::new();

        // Write header on first encode call
        if !self.header_written {
            output.extend_from_slice(&self.generate_wav_header());
            self.header_written = true;
        }

        // Convert and append audio data
        let audio_data = self.convert_samples(samples);
        output.extend_from_slice(&audio_data);

        self.metadata.samples_encoded += samples.len() as u64;
        self.metadata.bytes_written += audio_data.len() as u64;

        Ok(output)
    }

    fn finalize(&mut self) -> Result<Vec<u8>> {
        // A recording stopped before any samples arrived still needs a header to be a
        // readable, empty WAV rather than a zero-byte file
        if self.header_written {
            return Ok(Vec::new());
        }

        self.header_written = true;
        Ok(self.generate_wav_header())
    }

    fn file_extension(&self) -> &'static str {
        "wav"
    }

    fn get_metadata(&self) -> EncoderMetadata {
        self.metadata.clone()
    }

    fn finalize_patches(&self) -> Vec<(u64, Vec<u8>)> {
        if !self.header_written {
            return Vec::new();
        }

        // RIFF sizes are 32-bit, so a WAV cannot describe more than 4GiB
        let data_size = self.metadata.bytes_written.min(u32::MAX as u64) as u32;
        let riff_size = data_size.saturating_add(WAV_HEADER_LEN - 8);

        vec![
            (WAV_RIFF_SIZE_OFFSET, riff_size.to_le_bytes().to_vec()),
            (WAV_DATA_SIZE_OFFSET, data_size.to_le_bytes().to_vec()),
        ]
    }
}

/// MP3 encoder using LAME
pub struct Mp3Encoder {
    metadata: EncoderMetadata,
    bitrate: u32,
    initialized: bool,
    lame_encoder: Option<crate::audio::recording::lame::Lame>,
    /// The Xing header, once the encoder has finished and knows what to say
    ///
    /// Written over the frame LAME reserved at the front of the file. Held here
    /// because `finalize_patches` is asked afterwards and cannot re-derive it.
    tag_frame: Vec<u8>,
}

impl Mp3Encoder {
    /// Create a new MP3 encoder
    pub fn new() -> Self {
        Self {
            metadata: EncoderMetadata::default(),
            bitrate: 192,
            initialized: false,
            lame_encoder: None,
            tag_frame: Vec::new(),
        }
    }

    /// Configure encoder settings (simplified placeholder)
    fn configure_encoder(
        &mut self,
        config: &RecordingConfig,
        mp3_settings: &Mp3Settings,
    ) -> Result<()> {
        // TODO: Implement LAME encoder configuration when needed
        // For now, just store the configuration
        self.bitrate = mp3_settings.bitrate;

        info!(
            "MP3 encoder configured: {}Hz, {} channels, {}kbps",
            config.sample_rate, config.channels, mp3_settings.bitrate
        );

        Ok(())
    }
}

impl AudioEncoder for Mp3Encoder {
    fn initialize(&mut self, config: &RecordingConfig) -> Result<()> {
        // Extract MP3 settings from config
        let mp3_settings = config
            .format
            .mp3
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("MP3 settings not found in config"))?;

        self.metadata = EncoderMetadata {
            sample_rate: config.sample_rate,
            channels: config.channels,
            bit_depth: config.bit_depth,
            samples_encoded: 0,
            bytes_written: 0,
            encoder_name: Some(format!("MP3 LAME {}kbps", mp3_settings.bitrate)),
        };

        self.configure_encoder(config, mp3_settings)?;
        self.initialized = true;

        Ok(())
    }

    fn encode(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        if !self.initialized {
            return Err(anyhow::anyhow!("MP3 encoder not initialized"));
        }

        if samples.is_empty() {
            return Ok(Vec::new());
        }

        // Started on the first samples rather than at initialize: LAME fixes its
        // parameters when it starts, and the format is not settled until audio
        // is actually arriving.
        if self.lame_encoder.is_none() {
            let lame = crate::audio::recording::lame::Lame::new(
                self.metadata.sample_rate,
                self.metadata.channels,
                self.bitrate,
            )?;

            self.lame_encoder = Some(lame);
            info!(
                "LAME MP3 encoder initialized: {}Hz, {} channels, {}kbps",
                self.metadata.sample_rate, self.metadata.channels, self.bitrate
            );
        }

        let lame = self
            .lame_encoder
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("LAME encoder went away"))?;

        // Handed the interleaved floats as they are. Converting to i16 first
        // threw away the precision the mixer works in for no reason: LAME takes
        // float input and does its own conversion better.
        let encoded = lame.encode(samples)?;

        self.metadata.samples_encoded += samples.len() as u64;
        self.metadata.bytes_written += encoded.len() as u64;

        Ok(encoded)
    }

    /// End the file properly
    ///
    /// Two things the previous version did neither of. The flush emits the
    /// samples still inside the encoder — without it the last frames of every
    /// recording were simply lost — and the tag frame carries the frame count
    /// and seek table a player needs to scrub or report a duration.
    fn finalize(&mut self) -> Result<Vec<u8>> {
        let Some(mut lame) = self.lame_encoder.take() else {
            return Ok(Vec::new());
        };

        let tail = lame.flush()?;

        // Only meaningful after the flush: neither the frame count nor the seek
        // table exists until the encoder has finished.
        self.tag_frame = lame.tag_frame();

        self.metadata.bytes_written += tail.len() as u64;

        info!(
            "MP3 encoder finalized: {} bytes flushed, {} byte header",
            tail.len(),
            self.tag_frame.len()
        );

        Ok(tail)
    }

    /// The Xing header goes over the frame LAME reserved at the front
    fn finalize_patches(&self) -> Vec<(u64, Vec<u8>)> {
        if self.tag_frame.is_empty() {
            return Vec::new();
        }

        vec![(0, self.tag_frame.clone())]
    }

    fn file_extension(&self) -> &'static str {
        "mp3"
    }

    fn get_metadata(&self) -> EncoderMetadata {
        self.metadata.clone()
    }
}

/// FLAC encoder (placeholder for future implementation)
pub struct FlacEncoder {
    metadata: EncoderMetadata,
    compression_level: u8,
}

impl FlacEncoder {
    pub fn new() -> Self {
        Self {
            metadata: EncoderMetadata::default(),
            compression_level: 5,
        }
    }
}

impl AudioEncoder for FlacEncoder {
    fn initialize(&mut self, config: &RecordingConfig) -> Result<()> {
        let flac_settings = config
            .format
            .flac
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("FLAC settings not found in config"))?;

        self.metadata = EncoderMetadata {
            sample_rate: config.sample_rate,
            channels: config.channels,
            bit_depth: config.bit_depth,
            samples_encoded: 0,
            bytes_written: 0,
            encoder_name: Some(format!("FLAC Level {}", flac_settings.compression_level)),
        };
        self.compression_level = flac_settings.compression_level;

        // TODO: Initialize FLAC encoder when library is added
        warn!("FLAC encoder not yet implemented - falling back to WAV");

        Ok(())
    }

    fn encode(&mut self, _samples: &[f32]) -> Result<Vec<u8>> {
        // TODO: Implement FLAC encoding
        Err(anyhow::anyhow!("FLAC encoding not yet implemented"))
    }

    fn finalize(&mut self) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }

    fn file_extension(&self) -> &'static str {
        "flac"
    }

    fn get_metadata(&self) -> EncoderMetadata {
        self.metadata.clone()
    }
}

/// Encoder factory for creating appropriate encoders
pub struct EncoderFactory;

impl EncoderFactory {
    /// Create an encoder based on the recording configuration
    pub fn create_encoder(config: &RecordingConfig) -> Result<Box<dyn AudioEncoder>> {
        if config.format.mp3.is_some() {
            Ok(Box::new(Mp3Encoder::new()))
        } else if config.format.flac.is_some() {
            Ok(Box::new(FlacEncoder::new()))
        } else {
            // Default to WAV
            Ok(Box::new(WavEncoder::new()))
        }
    }

    /// Get list of supported formats
    pub fn supported_formats() -> Vec<&'static str> {
        vec!["wav", "mp3"] // FLAC when implemented
    }

    /// Check if a format is supported
    pub fn is_format_supported(extension: &str) -> bool {
        Self::supported_formats().contains(&extension.to_lowercase().as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::recording::types::*;

    #[test]
    fn test_wav_encoder_initialization() {
        let mut encoder = WavEncoder::new();
        let config = RecordingConfig::default();

        assert!(encoder.initialize(&config).is_ok());
        assert_eq!(encoder.file_extension(), "wav");

        let metadata = encoder.get_metadata();
        assert_eq!(metadata.sample_rate, config.sample_rate);
        assert_eq!(metadata.channels, config.channels);
    }

    fn wav_encoder_for(bit_depth: u16) -> WavEncoder {
        let mut encoder = WavEncoder::new();
        encoder
            .initialize(&RecordingConfig {
                sample_rate: 48000,
                channels: 2,
                bit_depth,
                ..Default::default()
            })
            .unwrap();
        encoder
    }

    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    fn read_u16(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
    }

    #[test]
    fn test_wav_header_matches_canonical_layout() {
        let mut encoder = wav_encoder_for(24);
        let output = encoder.encode(&[0.0; 4]).unwrap();
        let header = &output[..WAV_HEADER_LEN as usize];

        assert_eq!(&header[0..4], b"RIFF");
        assert_eq!(&header[8..12], b"WAVE");
        assert_eq!(&header[12..16], b"fmt ");
        assert_eq!(read_u32(header, 16), 16, "fmt chunk size");
        assert_eq!(read_u16(header, 20), 1, "PCM format tag");
        assert_eq!(read_u16(header, 22), 2, "channels");
        assert_eq!(read_u32(header, 24), 48000, "sample rate");
        assert_eq!(read_u32(header, 28), 48000 * 2 * 3, "byte rate");
        assert_eq!(read_u16(header, 32), 6, "block align");
        assert_eq!(read_u16(header, 34), 24, "bit depth");
        assert_eq!(&header[36..40], b"data");

        // The size fields live where the patch offsets say they do
        assert_eq!(WAV_RIFF_SIZE_OFFSET, 4);
        assert_eq!(WAV_DATA_SIZE_OFFSET as usize, WAV_HEADER_LEN as usize - 4);
    }

    #[test]
    fn test_wav_finalize_patches_describe_the_written_data() {
        let mut encoder = wav_encoder_for(16);
        encoder.encode(&[0.5; 100]).unwrap();
        encoder.finalize().unwrap();

        let patches = encoder.finalize_patches();
        assert_eq!(patches.len(), 2);

        let data_bytes = 100 * 2;
        assert_eq!(patches[0].0, WAV_RIFF_SIZE_OFFSET);
        assert_eq!(
            read_u32(&patches[0].1, 0),
            data_bytes + WAV_HEADER_LEN - 8,
            "RIFF size covers everything after the size field itself"
        );
        assert_eq!(patches[1].0, WAV_DATA_SIZE_OFFSET);
        assert_eq!(read_u32(&patches[1].1, 0), data_bytes, "data size");
    }

    #[test]
    fn test_wav_finalize_emits_header_when_no_samples_were_encoded() {
        let mut encoder = wav_encoder_for(16);

        let trailing = encoder.finalize().unwrap();
        assert_eq!(trailing.len(), WAV_HEADER_LEN as usize);
        assert_eq!(&trailing[0..4], b"RIFF");

        let patches = encoder.finalize_patches();
        assert_eq!(read_u32(&patches[0].1, 0), WAV_HEADER_LEN - 8);
        assert_eq!(read_u32(&patches[1].1, 0), 0);
    }

    #[test]
    fn test_wav_sample_conversion_widths_match_bit_depth() {
        for (bit_depth, bytes_per_sample) in [(16, 2), (24, 3), (32, 4)] {
            let mut encoder = wav_encoder_for(bit_depth);
            let output = encoder.encode(&[0.0; 8]).unwrap();
            assert_eq!(
                output.len(),
                WAV_HEADER_LEN as usize + 8 * bytes_per_sample,
                "{}-bit sample width",
                bit_depth
            );
        }
    }

    #[test]
    fn test_wav_full_scale_samples_stay_in_range() {
        // Every depth is integer PCM, so full-scale input must land on the type's
        // extremes rather than wrapping around to the opposite sign
        let mut encoder = wav_encoder_for(16);
        let output = encoder.encode(&[1.0, -1.0]).unwrap();
        let data = &output[WAV_HEADER_LEN as usize..];
        assert_eq!(read_u16(data, 0) as i16, 32767);
        assert_eq!(read_u16(data, 2) as i16, -32767);

        let mut encoder = wav_encoder_for(32);
        let output = encoder.encode(&[1.0, -1.0]).unwrap();
        let data = &output[WAV_HEADER_LEN as usize..];
        assert_eq!(read_u32(data, 0) as i32, i32::MAX);
        assert_eq!(read_u32(data, 4) as i32, -i32::MAX);
    }

    #[test]
    fn test_encoder_factory() {
        let wav_config = RecordingConfig {
            format: RecordingFormat {
                wav: Some(WavSettings {}),
                mp3: None,
                flac: None,
            },
            ..Default::default()
        };

        let encoder = EncoderFactory::create_encoder(&wav_config).unwrap();
        assert_eq!(encoder.file_extension(), "wav");

        assert!(EncoderFactory::is_format_supported("wav"));
        assert!(EncoderFactory::is_format_supported("mp3"));
        assert!(!EncoderFactory::is_format_supported("ogg"));
    }
}

#[cfg(test)]
mod mp3_tests {
    use super::*;
    use crate::audio::recording::types::{Mp3Settings, RecordingConfig, RecordingFormat};

    fn config() -> RecordingConfig {
        RecordingConfig {
            sample_rate: 48_000,
            channels: 2,
            bit_depth: 16,
            format: RecordingFormat {
                mp3: Some(Mp3Settings { bitrate: 192 }),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// The bug from #123
    ///
    /// Finalize used to drop the encoder and return nothing, so the samples
    /// still inside LAME never reached the file and no header was ever written.
    #[test]
    fn finalizing_flushes_the_encoder_and_writes_a_header() {
        let mut encoder = Mp3Encoder::new();
        encoder.initialize(&config()).expect("initializes");

        // A second of a tone, so there is definitely something to lose.
        let samples: Vec<f32> = (0..48_000 * 2)
            .map(|n| ((n as f32) * 0.01).sin() * 0.25)
            .collect();

        let body = encoder.encode(&samples).expect("encodes");
        let tail = encoder.finalize().expect("finalizes");
        let patches = encoder.finalize_patches();

        assert!(!body.is_empty(), "the audio encodes to frames");
        assert!(!tail.is_empty(), "finalize returns the flushed remainder");

        assert_eq!(patches.len(), 1, "one patch, for the header");
        assert_eq!(patches[0].0, 0, "written at the front of the file");

        let header = &patches[0].1;
        assert_eq!(header[0], 0xFF, "the header is an MPEG frame");
        assert!(
            header
                .windows(4)
                .any(|window| window == b"Info" || window == b"Xing"),
            "the header carries the seek table"
        );
    }

    /// Recording nothing should still close cleanly rather than erroring
    #[test]
    fn finalizing_without_audio_is_not_an_error() {
        let mut encoder = Mp3Encoder::new();
        encoder.initialize(&config()).expect("initializes");

        assert!(encoder.finalize().expect("finalizes").is_empty());
        assert!(encoder.finalize_patches().is_empty());
    }
}
