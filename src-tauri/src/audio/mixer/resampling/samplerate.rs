use anyhow::Result;
use colored::*;
use samplerate_rs::{convert, ConverterType};
use tracing::info;
/// Sample Rate Converter for Dynamic Audio Buffer Conversion
///
/// Handles real-time sample rate conversion between input and output devices
/// Supports both upsampling (interpolation) and downsampling (decimation)
/// Optimized for low-latency audio processing in callback contexts

/// libsamplerate-based sample rate converter using the samplerate crate
/// Provides high-quality audio resampling with multiple converter types
/// More flexible than rubato for variable input sizes
#[derive(Debug)]
pub struct SamplerateSRC {
    /// Input sample rate
    pub input_rate: u32,
    /// Output sample rate
    pub output_rate: u32,
    /// Conversion ratio (output_rate / input_rate)
    ratio: f32,
    /// Converter type for quality/performance trade-off
    converter_type: ConverterType,
    /// Number of channels (2 for stereo)
    channels: usize,
}

impl SamplerateSRC {
    /// Create a new libsamplerate-based converter
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz (e.g., 48000)
    /// * `output_rate` - Output sample rate in Hz (e.g., 44100)
    /// * `converter_type` - Quality/performance trade-off:
    ///   - `SincBestQuality`: Highest quality, slowest
    ///   - `SincMediumQuality`: Good quality, moderate speed
    ///   - `SincFastest`: Lower quality, fastest
    ///   - `ZeroOrderHold`: Lowest quality, very fast
    ///   - `Linear`: Linear interpolation, fast
    ///
    /// # Returns
    /// libsamplerate-based converter that can handle variable input sizes
    pub fn new(
        input_rate: u32,
        output_rate: u32,
        converter_type: ConverterType,
    ) -> Result<Self, String> {
        info!(
            "🎯 {}: Creating libsamplerate converter {}Hz→{}Hz with {:?}",
            "SAMPLERATE_INIT".blue(),
            input_rate,
            output_rate,
            converter_type
        );

        let ratio = output_rate as f32 / input_rate as f32;

        info!(
            "🎯 {}: libsamplerate configured with ratio {:.3}",
            "SAMPLERATE_INIT".blue(),
            ratio
        );

        Ok(Self {
            input_rate,
            output_rate,
            ratio,
            converter_type,
            channels: 2, // Stereo
        })
    }

    /// Convert input samples to output sample rate using libsamplerate
    /// Can handle variable input sizes, unlike rubato's fixed input requirement
    ///
    /// # Arguments
    /// * `input_samples` - Input audio samples at input_rate (interleaved stereo)
    ///
    /// # Returns
    /// Vector of resampled audio with length determined by conversion ratio
    pub fn convert(&mut self, input_samples: &[f32]) -> Vec<f32> {
        // Handle empty input
        if input_samples.is_empty() {
            return Vec::new();
        }

        // Perform resampling using libsamplerate
        match convert(
            self.input_rate,
            self.output_rate,
            self.channels,
            self.converter_type,
            input_samples,
        ) {
            Ok(output_samples) => {
                // info!(
                //     "🎯 {}: Converted {} → {} samples (ratio: {:.3})",
                //     "SAMPLERATE_CONVERT".green(),
                //     input_samples.len(),
                //     output_samples.len(),
                //     output_samples.len() as f32 / input_samples.len() as f32
                // );
                output_samples
            }
            Err(e) => {
                info!("❌ {}: Conversion failed: {}", "SAMPLERATE_ERROR".red(), e);
                Vec::new()
            }
        }
    }

    /// Get conversion ratio (for debugging and compatibility)
    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    /// Check if conversion is needed (rates are different)
    pub fn conversion_needed(&self) -> bool {
        (self.ratio - 1.0).abs() > 0.001
    }

    /// Get delay introduced by the resampler (for latency compensation)
    /// Note: libsamplerate has minimal delay compared to FFT-based methods
    pub fn output_delay(&self) -> f32 {
        // libsamplerate has very low latency, especially for simple converters
        match self.converter_type {
            ConverterType::SincBestQuality => 10.0, // Roughly 10 samples delay
            ConverterType::SincMediumQuality => 5.0, // Roughly 5 samples delay
            ConverterType::SincFastest => 2.0,      // Roughly 2 samples delay
            ConverterType::ZeroOrderHold => 0.0,    // No delay
            ConverterType::Linear => 1.0,           // Minimal delay
        }
    }

    /// Calculate number of input frames needed to produce desired output frames
    /// This provides consistent API across all resampler implementations
    ///
    /// # Arguments
    /// * `desired_output_frames` - Number of output frames desired
    ///
    /// # Returns
    /// Number of input frames needed (estimated based on conversion ratio)
    pub fn input_frames_needed(&self, desired_output_frames: usize) -> usize {
        // For libsamplerate, estimate input frames needed based on conversion ratio
        // libsamplerate is very predictable, so we can be quite accurate
        let estimated_input = (desired_output_frames as f32 / self.ratio).ceil() as usize;

        // Add small buffer for safety
        (estimated_input as f32 * 1.05).ceil() as usize
    }

    /// Update sample rates (useful for dynamic rate changes)
    pub fn update_rates(&mut self, input_rate: u32, output_rate: u32) {
        self.input_rate = input_rate;
        self.output_rate = output_rate;
        self.ratio = output_rate as f32 / input_rate as f32;

        info!(
            "🎯 {}: Updated rates to {}Hz→{}Hz (ratio: {:.3})",
            "SAMPLERATE_UPDATE".cyan(),
            input_rate,
            output_rate,
            self.ratio
        );
    }
}
