use anyhow::Result;
use colored::*;
use rubato::{FftFixedIn, Resampler as RubatoResampler};
use samplerate_rs::{convert, ConverterType};
use tracing::info;
/// Sample Rate Converter for Dynamic Audio Buffer Conversion
///
/// Handles real-time sample rate conversion between input and output devices
/// Supports both upsampling (interpolation) and downsampling (decimation)
/// Optimized for low-latency audio processing in callback contexts

/// FFT-based fixed input resampler - simplified single implementation
struct ResamplerWrapper {
    /// FFT-based fixed input resampler (variable output)
    resampler: FftFixedIn<f32>,
}

impl std::fmt::Debug for ResamplerWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ResamplerWrapper(FftFixedIn)")
    }
}

/// FFT-based sample rate converter using FftFixedIn
/// Provides good quality audio resampling with fixed input size and variable output size
/// Optimized for real-time audio processing with pre-allocated buffers
#[derive(Debug)]
pub struct RubatoSRC {
    /// FFT-based fixed input resampler
    resampler: ResamplerWrapper,
    /// Pre-allocated input buffer for resampler
    input_buffer: Vec<Vec<f32>>,
    /// Pre-allocated output buffer for resampler (sized for maximum possible output)
    output_buffer: Vec<Vec<f32>>,
    /// Input sample rate
    pub input_rate: f32,
    /// Output sample rate
    pub output_rate: f32,
    /// Conversion ratio (output_rate / input_rate)
    ratio: f32,
    /// Fixed input chunk size this resampler expects
    input_frames: usize,
    /// **PERFORMANCE FIX**: Reusable result buffer to eliminate Vec allocations
    reusable_result_buffer: Vec<f32>,
}

impl RubatoSRC {
    /// Create a FFT-based fixed input resampler using FftFixedIn
    /// This accepts a fixed number of input frames and returns a variable number of output frames
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz (e.g., 48000)
    /// * `output_rate` - Output sample rate in Hz (e.g., 44100)
    /// * `chunk_size_in` - Fixed number of input frames per call (e.g., 512)
    ///
    /// # Returns
    /// FFT-based resampler with fixed input and variable output frame sizes
    pub fn new_fft_fixed_input(
        input_rate: f32,
        output_rate: f32,
        chunk_size_in: usize,
    ) -> Result<Self, String> {
        info!(
            "🎯 {}: Creating FftFixedIn resampler {}Hz→{}Hz with {} input frames",
            "FFT_FIXED_IN".blue(),
            input_rate,
            output_rate,
            chunk_size_in
        );

        // Create Rubato's FFT-based fixed input resampler
        let resampler = FftFixedIn::new(
            input_rate as usize,  // sample_rate_input
            output_rate as usize, // sample_rate_output
            chunk_size_in,        // chunk_size_in: fixed input chunk size in frames
            4,                    // sub_chunks: desired number of subchunks for processing
            2,                    // nbr_channels: number of channels (stereo)
        )
        .map_err(|e| format!("Failed to create FftFixedIn resampler: {}", e))?;

        // Get the maximum possible output frame count from the resampler
        let max_output_frames = resampler.output_frames_max();

        info!(
            "🎯 {}: FftFixedIn configured: {} input frames → max {} output frames (ratio: {:.3})",
            "FFT_FIXED_IN".blue(),
            chunk_size_in,
            max_output_frames,
            output_rate / input_rate
        );

        // Pre-allocate buffers for zero-allocation processing
        let input_buffer = vec![vec![0.0; chunk_size_in]; 2]; // 2 channels for stereo
        let output_buffer = vec![vec![0.0; max_output_frames]; 2]; // Maximum possible output size

        Ok(Self {
            resampler: ResamplerWrapper { resampler },
            input_buffer,
            output_buffer,
            input_rate,
            output_rate,
            ratio: output_rate / input_rate,
            input_frames: chunk_size_in,
            reusable_result_buffer: Vec::with_capacity(max_output_frames * 2), // Stereo samples
        })
    }

    /// Convert input samples to output sample rate using FftFixedIn
    /// Accepts exactly the fixed input frame size and returns variable output frames
    ///
    /// # Arguments
    /// * `input_samples` - Input audio samples at input_rate (interleaved stereo)
    ///
    /// # Returns
    /// Vector of resampled audio with variable length determined by FftFixedIn
    pub fn convert(&mut self, input_samples: &[f32]) -> Vec<f32> {
        // Handle empty input
        if input_samples.is_empty() {
            return Vec::new();
        }

        // FftFixedIn requires exactly the configured input frame count
        let input_frames = input_samples.len() / 2; // Each frame has L+R samples

        // Ensure we have exactly the required input frames, pad with zeros if needed
        let frames_to_process = self.input_frames;

        // Clear the input buffer
        for i in 0..frames_to_process {
            self.input_buffer[0][i] = 0.0;
            self.input_buffer[1][i] = 0.0;
        }

        // De-interleave input samples: LRLRLR... -> L...L, R...R
        // Fill actual data up to available frames, pad with zeros if needed
        let frames_available = input_frames.min(frames_to_process);
        for frame in 0..frames_available {
            if frame * 2 + 1 < input_samples.len() {
                self.input_buffer[0][frame] = input_samples[frame * 2]; // Left channel
                self.input_buffer[1][frame] = input_samples[frame * 2 + 1]; // Right channel
            }
        }

        // Perform resampling using FftFixedIn
        let process_result = self.resampler.resampler.process_into_buffer(
            &self.input_buffer,
            &mut self.output_buffer,
            None,
        );

        match process_result {
            Ok((_input_frames_used, output_frames_generated)) => {
                // Interleave output: L...L, R...R -> LRLRLR...
                self.reusable_result_buffer.clear();
                self.reusable_result_buffer
                    .reserve(output_frames_generated * 2);

                for frame in 0..output_frames_generated {
                    self.reusable_result_buffer
                        .push(self.output_buffer[0][frame]); // Left
                    self.reusable_result_buffer
                        .push(self.output_buffer[1][frame]); // Right
                }

                self.reusable_result_buffer.clone()
            }
            Err(e) => {
                info!(
                    "❌ {}: Resampling failed: {}",
                    "FFT_FIXED_IN_ERROR".red(),
                    e
                );
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
    pub fn output_delay(&self) -> f32 {
        self.resampler.resampler.output_delay() as f32
    }

    /// Get the fixed input frame size
    pub fn get_input_frames(&self) -> usize {
        self.input_frames
    }
}
