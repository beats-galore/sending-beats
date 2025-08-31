// Audio encoding implementations for different formats
//
// This module provides audio encoders for WAV, MP3, and FLAC formats.
// Each encoder implements the AudioEncoder trait for consistent interface
// while handling format-specific encoding requirements.

use anyhow::Result;
use tracing::{info, error, warn};

use super::types::{RecordingConfig, Mp3Settings, FlacSettings};

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
        
        // RIFF header
        header.extend_from_slice(b"RIFF");
        header.extend_from_slice(&[0, 0, 0, 0]); // File size placeholder
        header.extend_from_slice(b"WAVE");
        
        // fmt chunk
        header.extend_from_slice(b"fmt ");
        header.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        header.extend_from_slice(&1u16.to_le_bytes());  // PCM format
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
                    output.extend_from_slice(&sample.to_le_bytes());
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
        
        info!("WAV encoder initialized: {}Hz, {} channels, {} bit", 
              config.sample_rate, config.channels, config.bit_depth);
        Ok(())
    }
    
    fn encode(&mut self, samples: &[f32]) -> Result<Vec<u8> > {
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
        // WAV doesn't need special finalization - header updates would be handled by writer
        Ok(Vec::new())
    }
    
    fn file_extension(&self) -> &'static str {
        "wav"
    }
    
    fn get_metadata(&self) -> EncoderMetadata {
        self.metadata.clone()
    }
}

/// MP3 encoder using LAME
pub struct Mp3Encoder {
    metadata: EncoderMetadata,
    bitrate: u32,
    initialized: bool,
    lame_encoder: Option<lame::Lame>,
}

// SAFETY: LAME encoder is used single-threaded within the recording writer task
unsafe impl Send for Mp3Encoder {}

impl Mp3Encoder {
    /// Create a new MP3 encoder
    pub fn new() -> Self {
        Self {
            metadata: EncoderMetadata::default(),
            bitrate: 192,
            initialized: false,
            lame_encoder: None,
        }
    }
    
    /// Configure encoder settings (simplified placeholder)
    fn configure_encoder(&mut self, config: &RecordingConfig, mp3_settings: &Mp3Settings) -> Result<()> {
        // TODO: Implement LAME encoder configuration when needed
        // For now, just store the configuration
        self.bitrate = mp3_settings.bitrate;
        
        info!("MP3 encoder configured: {}Hz, {} channels, {}kbps", 
              config.sample_rate, config.channels, mp3_settings.bitrate);
        
        Ok(())
    }
    
    /// Convert interleaved f32 samples to separate left/right channels for LAME
    fn separate_channels(&self, samples: &[f32]) -> (Vec<f32>, Vec<f32>) {
        if self.metadata.channels == 1 {
            (samples.to_vec(), Vec::new())
        } else {
            let mut left = Vec::with_capacity(samples.len() / 2);
            let mut right = Vec::with_capacity(samples.len() / 2);
            
            for chunk in samples.chunks_exact(2) {
                left.push(chunk[0]);
                right.push(chunk[1]);
            }
            
            (left, right)
        }
    }
}

impl AudioEncoder for Mp3Encoder {
    fn initialize(&mut self, config: &RecordingConfig) -> Result<()> {
        // Extract MP3 settings from config
        let mp3_settings = config.format.mp3.as_ref()
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
        
        // Initialize LAME encoder on first use
        if self.lame_encoder.is_none() {
            let mut lame = lame::Lame::new()
                .ok_or_else(|| anyhow::anyhow!("Failed to create LAME encoder"))?;
            
            lame.set_channels(self.metadata.channels as u8)
                .map_err(|_| anyhow::anyhow!("Failed to set LAME channels"))?;
            lame.set_sample_rate(self.metadata.sample_rate)
                .map_err(|_| anyhow::anyhow!("Failed to set LAME sample rate"))?;
            lame.set_kilobitrate(self.bitrate as i32)
                .map_err(|_| anyhow::anyhow!("Failed to set LAME bitrate"))?;
            lame.set_quality(2) // Good quality balance
                .map_err(|_| anyhow::anyhow!("Failed to set LAME quality"))?;
            
            lame.init_params()
                .map_err(|_| anyhow::anyhow!("Failed to initialize LAME parameters"))?;
            
            self.lame_encoder = Some(lame);
            info!("LAME MP3 encoder initialized: {}Hz, {} channels, {}kbps", 
                  self.metadata.sample_rate, self.metadata.channels, self.bitrate);
        }
        
        let lame = self.lame_encoder.as_mut().unwrap();
        
        // Convert f32 samples to i16 for LAME
        let samples_i16: Vec<i16> = samples.iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();
        
        // Create MP3 buffer (LAME recommended size)
        let mut mp3_buffer = vec![0u8; samples_i16.len() * 2 + 7200];
        
        let encoded_size = if self.metadata.channels == 1 {
            lame.encode(&samples_i16, &[], &mut mp3_buffer)
                .map_err(|e| anyhow::anyhow!("LAME mono encoding error: {:?}", e))?
        } else {
            // For stereo, split interleaved samples into left/right channels
            let left: Vec<i16> = samples_i16.iter().step_by(2).copied().collect();
            let right: Vec<i16> = samples_i16.iter().skip(1).step_by(2).copied().collect();
            
            lame.encode(&left, &right, &mut mp3_buffer)
                .map_err(|e| anyhow::anyhow!("LAME stereo encoding error: {:?}", e))?
        };
        
        self.metadata.samples_encoded += samples.len() as u64;
        self.metadata.bytes_written += encoded_size as u64;
        
        if encoded_size > 0 {
            mp3_buffer.truncate(encoded_size);
            Ok(mp3_buffer)
        } else {
            Ok(Vec::new())
        }
    }
    
    fn finalize(&mut self) -> Result<Vec<u8>> {
        if let Some(_lame) = self.lame_encoder.take() {
            // MP3 finalization - LAME crate doesn't have flush method
            // The working version just returned empty buffer for MP3 finalization
            info!("MP3 encoder finalized");
            Ok(Vec::new())
        } else {
            Ok(Vec::new())
        }
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
        let flac_settings = config.format.flac.as_ref()
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