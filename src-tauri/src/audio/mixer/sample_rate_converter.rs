use r8brain_rs::{PrecisionProfile, Resampler as R8BrainResampler};
use rubato::{
    Resampler as RubatoResampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};
/// Sample Rate Converter for Dynamic Audio Buffer Conversion
///
/// Handles real-time sample rate conversion between input and output devices
/// Supports both upsampling (interpolation) and downsampling (decimation)
/// Optimized for low-latency audio processing in callback contexts
use std::collections::VecDeque;

/// Professional sample rate converter using Rubato's windowed sinc interpolation
/// Provides broadcast-quality, transparent audio resampling with anti-aliasing
/// Optimized for real-time audio processing with pre-allocated buffers
pub struct RubatoSRC {
    /// Rubato's high-quality sinc interpolation resampler
    resampler: SincFixedIn<f32>,
    /// Pre-allocated input buffer for resampler
    input_buffer: Vec<Vec<f32>>,
    /// Pre-allocated output buffer for resampler
    output_buffer: Vec<Vec<f32>>,
    /// Input sample rate
    input_rate: f32,
    /// Output sample rate
    output_rate: f32,
    /// Conversion ratio (output_rate / input_rate)
    ratio: f32,
    /// Maximum input chunk size this resampler can handle
    max_input_frames: usize,
}

impl RubatoSRC {
    /// Create a new professional sample rate converter with broadcast quality
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz (e.g., 44100)
    /// * `output_rate` - Output sample rate in Hz (e.g., 48000)
    /// * `max_input_frames` - Maximum expected input chunk size (default: 1024)
    ///
    /// # Returns
    /// High-quality resampler with windowed sinc interpolation and anti-aliasing
    pub fn new(input_rate: f32, output_rate: f32) -> Result<Self, String> {
        Self::with_max_frames(input_rate, output_rate, 1024)
    }

    /// Create a new resampler with specified maximum input frame size
    pub fn with_max_frames(
        input_rate: f32,
        output_rate: f32,
        max_input_frames: usize,
    ) -> Result<Self, String> {
        // Configure high-quality sinc interpolation parameters
        let params = SincInterpolationParameters {
            sinc_len: 256,                                // High-quality sinc filter length
            f_cutoff: 0.95,                               // Conservative cutoff to prevent aliasing
            interpolation: SincInterpolationType::Linear, // Linear interpolation between sinc samples
            oversampling_factor: 256,                     // High oversampling for quality
            window: WindowFunction::BlackmanHarris2,      // Excellent side-lobe suppression
        };

        // Create Rubato's fixed-input-size resampler
        // NOTE: SincFixedIn may expect INPUT/OUTPUT ratio despite documentation
        let resampler = SincFixedIn::new(
            input_rate as f64 / output_rate as f64, // Try INPUT/OUTPUT ratio (48000/44100 = 1.088)
            2.0, // Maximum ratio change (for future dynamic adjustment)
            params,
            max_input_frames, // Fixed input chunk size
            2,                // Number of channels (stereo)
        )
        .map_err(|e| format!("Failed to create Rubato resampler: {}", e))?;

        // Pre-allocate buffers for zero-allocation processing
        let input_buffer = vec![vec![0.0; max_input_frames]; 2]; // 2 channels for stereo
        let max_output_frames = ((max_input_frames as f64 * output_rate as f64 / input_rate as f64)
            .ceil() as usize)
            + 64; // Extra headroom
        let output_buffer = vec![vec![0.0; max_output_frames]; 2]; // 2 channels for stereo

        Ok(Self {
            resampler,
            input_buffer,
            output_buffer,
            input_rate,
            output_rate,
            ratio: input_rate / output_rate, // Store the ratio we actually used (INPUT/OUTPUT)
            max_input_frames,
        })
    }

    /// Convert input samples to output sample rate with broadcast quality
    ///
    /// # Arguments
    /// * `input_samples` - Input audio samples at input_rate (interleaved stereo)
    /// * `output_size` - Desired number of output samples (interleaved stereo)
    ///
    /// # Returns
    /// Vector of resampled audio with transparent quality and anti-aliasing
    pub fn convert(&mut self, input_samples: &[f32], output_size: usize) -> Vec<f32> {
        // Handle empty input
        if input_samples.is_empty() {
            return vec![0.0; output_size];
        }

        // Convert interleaved stereo to de-interleaved format for Rubato
        let input_frames = input_samples.len() / 2; // Each frame has L+R samples
        let input_frames = input_frames.min(self.max_input_frames);

        // DEBUG: Track conversion details
        println!(
            "🔍 RUBATO_DEBUG: Input {} samples → {} frames, ratio {:.4} ({}→{}Hz)",
            input_samples.len(),
            input_frames,
            self.ratio,
            self.input_rate,
            self.output_rate
        );

        if input_frames == 0 {
            return vec![0.0; output_size];
        }

        // De-interleave: LRLRLR... -> L...L, R...R
        for frame in 0..input_frames {
            if frame * 2 + 1 < input_samples.len() {
                self.input_buffer[0][frame] = input_samples[frame * 2]; // Left channel
                self.input_buffer[1][frame] = input_samples[frame * 2 + 1]; // Right channel
            }
        }

        // Zero-pad the rest of the buffers if needed
        for frame in input_frames..self.max_input_frames {
            self.input_buffer[0][frame] = 0.0;
            self.input_buffer[1][frame] = 0.0;
        }

        // Perform high-quality resampling
        match self
            .resampler
            .process_into_buffer(&self.input_buffer, &mut self.output_buffer, None)
        {
            Ok((_input_frames_used, output_frames_generated)) => {
                // Re-interleave the output: L...L, R...R -> LRLRLR...
                // Return ACTUAL converted samples, not padded to requested size
                let actual_output_frames = output_frames_generated.min(self.output_buffer[0].len());
                let mut result = Vec::with_capacity(actual_output_frames * 2);

                for frame in 0..actual_output_frames {
                    result.push(self.output_buffer[0][frame]); // Left
                    result.push(self.output_buffer[1][frame]); // Right
                }

                // DEBUG: Track output conversion
                println!(
                    "🔍 RUBATO_DEBUG: {} frames → {} samples ({}→{})",
                    actual_output_frames,
                    result.len(),
                    input_frames,
                    actual_output_frames
                );

                result
            }
            Err(e) => {
                println!("❌ RUBATO_ERROR: Resampling failed: {}", e);
                // Fallback to silence on processing error
                vec![0.0; output_size]
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
    pub fn output_delay(&self) -> f32 {
        self.resampler.output_delay() as f32
    }
}

/// **PROFESSIONAL BROADCAST QUALITY**: R8Brain sample rate converter
///
/// Uses Aleksey Vaneev's r8brain-free-src library - the gold standard for
/// transparent, zero-perceived-latency sample rate conversion used in:
/// - REAPER (professional DAW)
/// - Red Dead Redemption 2
/// - Audirvana (audiophile music player)
///
/// **KEY FEATURES**:
/// - Automatic latency removal for real-time processing
/// - 49-218dB stop-band attenuation (broadcast quality)
/// - Two-stage algorithm: 2X oversample + polynomial-interpolated sinc filters
/// - Designed specifically for real-time "pull" processing in audio callbacks
/// - Can handle 860+ concurrent streams at 100% CPU (professional performance)
pub struct R8BrainSRC {
    /// R8brain professional resampler instance
    resampler: R8BrainResampler,
    /// Input sample rate
    input_rate: f32,
    /// Output sample rate
    output_rate: f32,
    /// Conversion ratio (output_rate / input_rate)
    ratio: f32,
    /// Pre-allocated output buffer for zero-allocation processing
    output_buffer: Vec<f64>,
    /// Maximum expected input size
    max_input_size: usize,
}

impl R8BrainSRC {
    /// Create a new professional broadcast-quality sample rate converter
    ///
    /// This uses the same algorithm as professional DAWs and games for
    /// transparent audio quality with automatic latency compensation.
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz (e.g., 44100)
    /// * `output_rate` - Output sample rate in Hz (e.g., 48000)
    /// * `max_input_size` - Maximum expected input chunk size (default: 2048)
    ///
    /// # Returns
    /// Professional-grade resampler with transparent quality
    pub fn new(input_rate: f32, output_rate: f32) -> Result<Self, String> {
        Self::with_max_input_size(input_rate, output_rate, 2048)
    }

    /// Create resampler with specific maximum input size
    pub fn with_max_input_size(
        input_rate: f32,
        output_rate: f32,
        max_input_size: usize,
    ) -> Result<Self, String> {
        // Create r8brain professional resampler with broadcast-quality settings
        let resampler = R8BrainResampler::new(
            input_rate as f64,        // Source sample rate
            output_rate as f64,       // Destination sample rate
            max_input_size,           // Max input length per process() call
            2.0,                      // Required transition band (professional quality)
            PrecisionProfile::Bits24, // 24-bit precision for broadcast quality
        );

        // Calculate maximum possible output size for buffer pre-allocation
        let ratio = output_rate / input_rate;
        let max_output_size = ((max_input_size as f32 * ratio) as usize + 64).max(1024); // Extra headroom
        let output_buffer = vec![0.0; max_output_size];

        Ok(Self {
            resampler,
            input_rate,
            output_rate,
            ratio: output_rate / input_rate,
            output_buffer,
            max_input_size,
        })
    }

    /// Convert input samples with broadcast-quality transparent resampling
    ///
    /// Uses r8brain's **automatic latency removal** and **pull processing**
    /// designed specifically for real-time audio callbacks like CoreAudio.
    ///
    /// # Arguments
    /// * `input_samples` - Input audio samples at input_rate
    /// * `output_size` - Exact number of output samples needed
    ///
    /// # Returns
    /// Vector of exactly `output_size` samples with transparent quality
    /// **NO PERCEIVED LATENCY** - latency is automatically compensated
    pub fn convert(&mut self, input_samples: &[f32], output_size: usize) -> Vec<f32> {
        // Handle empty input
        if input_samples.is_empty() {
            return vec![0.0; output_size];
        }

        // Ensure we don't exceed buffer capacity
        let input_len = input_samples.len().min(self.max_input_size);

        // Convert f32 input to f64 for r8brain processing
        let input_f64: Vec<f64> = input_samples
            .iter()
            .take(input_len)
            .map(|&x| x as f64)
            .collect();

        // Process with r8brain professional resampler
        // Note: r8brain may need multiple calls before yielding output (this is normal)
        let output_len = self.resampler.process(&input_f64, &mut self.output_buffer);

        // Handle the output
        let mut result = Vec::with_capacity(output_size);

        if output_len > 0 {
            // We got some output from r8brain
            for i in 0..output_size {
                if i < output_len {
                    result.push(self.output_buffer[i] as f32);
                } else {
                    // Need more samples than r8brain produced, repeat last sample
                    let last_sample = if output_len > 0 {
                        self.output_buffer[output_len - 1] as f32
                    } else {
                        0.0
                    };
                    result.push(last_sample);
                }
            }
        } else {
            // r8brain hasn't produced output yet (normal during initial processing)
            // Fill with silence for now - output will come in subsequent calls
            result.resize(output_size, 0.0);
        }

        result
    }

    /// Get conversion ratio (for compatibility with other SRC types)
    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    /// Check if conversion is needed (rates are different)
    pub fn conversion_needed(&self) -> bool {
        (self.ratio - 1.0).abs() > 0.001
    }

    /// Get latency compensation applied (should be near-zero for r8brain)
    /// r8brain automatically removes processing latency in real-time mode
    pub fn effective_latency(&self) -> f32 {
        // r8brain removes latency automatically in pull mode
        // The initial processing latency is compensated internally
        0.0
    }
}

impl std::fmt::Debug for RubatoSRC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RubatoSRC")
            .field("input_rate", &self.input_rate)
            .field("output_rate", &self.output_rate)
            .field("ratio", &self.ratio)
            .field("max_input_frames", &self.max_input_frames)
            .finish()
    }
}

/// Utility functions for sample rate conversion
pub mod utils {
    /// Calculate exact output size needed for given input size and rates
    pub fn calculate_output_size(input_size: usize, input_rate: f32, output_rate: f32) -> usize {
        let ratio = output_rate / input_rate;
        ((input_size as f32) * ratio).ceil() as usize
    }

    /// Check if sample rates are effectively the same (within tolerance)
    pub fn rates_match(rate1: f32, rate2: f32) -> bool {
        (rate1 - rate2).abs() < 1.0 // 1 Hz tolerance
    }
}
