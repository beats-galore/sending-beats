/// Sample Rate Converter for Dynamic Audio Buffer Conversion
///
/// Handles real-time sample rate conversion between input and output devices
/// Supports both upsampling (interpolation) and downsampling (decimation)
/// Optimized for low-latency audio processing in callback contexts

use std::collections::VecDeque;
use rubato::{SincFixedIn, SincInterpolationType, SincInterpolationParameters, WindowFunction, Resampler as RubatoResampler};
use r8brain_rs::{PrecisionProfile, Resampler as R8BrainResampler};

/// Linear interpolation-based sample rate converter
/// Suitable for real-time audio processing with minimal CPU overhead
pub struct LinearSRC {
    /// Input sample rate (Hz)
    input_rate: f32,
    /// Output sample rate (Hz)
    output_rate: f32,
    /// Conversion ratio (output_rate / input_rate)
    ratio: f32,
    /// Previous input sample for interpolation
    prev_sample: f32,
    /// Current fractional position in input stream
    phase: f32,
}

impl LinearSRC {
    /// Create a new sample rate converter
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz (e.g., 44100)
    /// * `output_rate` - Output sample rate in Hz (e.g., 48000)
    pub fn new(input_rate: f32, output_rate: f32) -> Self {
        Self {
            input_rate,
            output_rate,
            ratio: output_rate / input_rate,
            prev_sample: 0.0,
            phase: 0.0,
        }
    }

    /// Convert input samples to output sample rate
    ///
    /// # Arguments
    /// * `input_samples` - Input audio samples at input_rate
    /// * `output_size` - Exact number of output samples needed
    ///
    /// # Returns
    /// Vector of exactly `output_size` samples at output_rate
    pub fn convert(&mut self, input_samples: &[f32], output_size: usize) -> Vec<f32> {
        if input_samples.is_empty() {
            return vec![0.0; output_size];
        }

        let mut output = Vec::with_capacity(output_size);
        let input_len = input_samples.len() as f32;

        // Reset phase for each conversion to maintain timing sync
        self.phase = 0.0;

        for _ in 0..output_size {
            // Current integer position in input array
            let input_index = self.phase.floor() as usize;

            if input_index >= input_samples.len() {
                // Past end of input, use last sample or extrapolate
                let last_sample = input_samples.last().copied().unwrap_or(0.0);
                output.push(last_sample);
            } else if input_index + 1 >= input_samples.len() {
                // At last input sample, no interpolation needed
                output.push(input_samples[input_index]);
            } else {
                // Linear interpolation between input_samples[i] and input_samples[i+1]
                let current_sample = input_samples[input_index];
                let next_sample = input_samples[input_index + 1];
                let fraction = self.phase - self.phase.floor();

                let interpolated = current_sample + (next_sample - current_sample) * fraction;
                output.push(interpolated);
            }

            // Advance phase by input step size
            self.phase += 1.0 / self.ratio;
        }

        // Update previous sample for next conversion
        self.prev_sample = input_samples.last().copied().unwrap_or(0.0);

        output
    }

    /// Get conversion ratio (for debugging)
    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    /// Check if conversion is needed (rates are different)
    pub fn conversion_needed(&self) -> bool {
        (self.ratio - 1.0).abs() > 0.001 // Allow small tolerance
    }
}

/// High-quality cubic interpolation sample rate converter
/// More CPU intensive but better audio quality for critical applications
pub struct CubicSRC {
    input_rate: f32,
    output_rate: f32,
    ratio: f32,
    /// History buffer for cubic interpolation (needs 4 samples)
    history: VecDeque<f32>,
    phase: f32,
}

impl CubicSRC {
    pub fn new(input_rate: f32, output_rate: f32) -> Self {
        let mut history = VecDeque::with_capacity(4);
        // Initialize with zeros
        for _ in 0..4 {
            history.push_back(0.0);
        }

        Self {
            input_rate,
            output_rate,
            ratio: output_rate / input_rate,
            history,
            phase: 0.0,
        }
    }

    pub fn convert(&mut self, input_samples: &[f32], output_size: usize) -> Vec<f32> {
        if input_samples.is_empty() {
            return vec![0.0; output_size];
        }

        // Add input samples to history buffer
        for &sample in input_samples {
            if self.history.len() >= 4 {
                self.history.pop_front();
            }
            self.history.push_back(sample);
        }

        let mut output = Vec::with_capacity(output_size);

        for _ in 0..output_size {
            // Cubic interpolation using 4-point window
            let interpolated = self.cubic_interpolate(self.phase.fract());
            output.push(interpolated);

            self.phase += 1.0 / self.ratio;

            // Advance history if we've moved past integer positions
            while self.phase >= 1.0 {
                self.phase -= 1.0;
                if self.history.len() >= 4 {
                    self.history.pop_front();
                    self.history.push_back(0.0); // Pad with zero if no more input
                }
            }
        }

        output
    }

    /// Cubic interpolation between 4 points
    fn cubic_interpolate(&self, t: f32) -> f32 {
        if self.history.len() < 4 {
            return 0.0;
        }

        let y0 = self.history[0];
        let y1 = self.history[1];
        let y2 = self.history[2];
        let y3 = self.history[3];

        // Cubic interpolation formula
        let a0 = y3 - y2 - y0 + y1;
        let a1 = y0 - y1 - a0;
        let a2 = y2 - y0;
        let a3 = y1;

        a0 * t * t * t + a1 * t * t + a2 * t + a3
    }

    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    pub fn conversion_needed(&self) -> bool {
        (self.ratio - 1.0).abs() > 0.001
    }
}

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
    pub fn with_max_frames(input_rate: f32, output_rate: f32, max_input_frames: usize) -> Result<Self, String> {
        // Configure high-quality sinc interpolation parameters
        let params = SincInterpolationParameters {
            sinc_len: 256,           // High-quality sinc filter length
            f_cutoff: 0.95,          // Conservative cutoff to prevent aliasing
            interpolation: SincInterpolationType::Linear,  // Linear interpolation between sinc samples
            oversampling_factor: 256,  // High oversampling for quality
            window: WindowFunction::BlackmanHarris2,  // Excellent side-lobe suppression
        };

        // Create Rubato's fixed-input-size resampler
        let resampler = SincFixedIn::new(
            output_rate as f64 / input_rate as f64,  // Conversion ratio
            2.0,  // Maximum ratio change (for future dynamic adjustment)
            params,
            max_input_frames,  // Fixed input chunk size
            1,    // Number of channels (mono - we process each channel separately)
        ).map_err(|e| format!("Failed to create Rubato resampler: {}", e))?;

        // Pre-allocate buffers for zero-allocation processing
        let input_buffer = vec![vec![0.0; max_input_frames]; 1];  // 1 channel
        let max_output_frames = ((max_input_frames as f64 * output_rate as f64 / input_rate as f64).ceil() as usize) + 64; // Extra headroom
        let output_buffer = vec![vec![0.0; max_output_frames]; 1];

        Ok(Self {
            resampler,
            input_buffer,
            output_buffer,
            input_rate,
            output_rate,
            ratio: output_rate / input_rate,
            max_input_frames,
        })
    }

    /// Convert input samples to output sample rate with broadcast quality
    ///
    /// # Arguments
    /// * `input_samples` - Input audio samples at input_rate
    /// * `output_size` - Desired number of output samples
    ///
    /// # Returns
    /// Vector of resampled audio with transparent quality and anti-aliasing
    pub fn convert(&mut self, input_samples: &[f32], output_size: usize) -> Vec<f32> {
        // Handle empty input
        if input_samples.is_empty() {
            return vec![0.0; output_size];
        }

        // Ensure input doesn't exceed our buffer capacity
        let input_len = input_samples.len().min(self.max_input_frames);

        // Copy input samples to resampler buffer
        self.input_buffer[0][..input_len].copy_from_slice(&input_samples[..input_len]);

        // If input is smaller than buffer, zero-pad the rest
        if input_len < self.max_input_frames {
            for sample in &mut self.input_buffer[0][input_len..] {
                *sample = 0.0;
            }
        }

        // Perform high-quality resampling
        match self.resampler.process_into_buffer(&self.input_buffer, &mut self.output_buffer, None) {
            Ok((_input_frames_used, output_frames_generated)) => {
                // Extract the exact number of samples requested
                let mut result = Vec::with_capacity(output_size);
                let available_samples = output_frames_generated.min(self.output_buffer[0].len());

                for i in 0..output_size {
                    if i < available_samples {
                        result.push(self.output_buffer[0][i]);
                    } else {
                        // If we need more samples than generated, repeat the last sample or use silence
                        result.push(if available_samples > 0 {
                            self.output_buffer[0][available_samples - 1]
                        } else {
                            0.0
                        });
                    }
                }

                result
            }
            Err(_) => {
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
    pub fn with_max_input_size(input_rate: f32, output_rate: f32, max_input_size: usize) -> Result<Self, String> {
        // Create r8brain professional resampler with broadcast-quality settings
        let resampler = R8BrainResampler::new(
            input_rate as f64,           // Source sample rate
            output_rate as f64,          // Destination sample rate
            max_input_size,              // Max input length per process() call
            2.0,                         // Required transition band (professional quality)
            PrecisionProfile::Bits24     // 24-bit precision for broadcast quality
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
        let input_f64: Vec<f64> = input_samples.iter().take(input_len).map(|&x| x as f64).collect();

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