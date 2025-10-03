use anyhow::Result;
use colored::*;
use rubato::{
    Resampler as RubatoResampler, SincFixedOut, SincInterpolationParameters, SincInterpolationType,
};
use tracing::info;
/// Sample Rate Converter for Dynamic Audio Buffer Conversion
///
/// Handles real-time sample rate conversion between input and output devices
/// Supports both upsampling (interpolation) and downsampling (decimation)
/// Optimized for low-latency audio processing in callback contexts

/// Sinc-based sample rate converter with dynamic ratio adjustment
/// Provides high quality audio resampling with adjustable ratio for clock synchronization
/// Optimized for real-time audio processing with pre-allocated buffers
pub struct RubatoSRC {
    /// Sinc-based resampler with dynamic ratio adjustment
    resampler: SincFixedOut<f32>,
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
    /// Max input frames this resampler can accept
    input_frames: usize,
    /// Fixed output chunk size this resampler produces
    output_frames: usize,
    /// **PERFORMANCE FIX**: Reusable result buffer to eliminate Vec allocations
    reusable_result_buffer: Vec<f32>,
    /// Identifier for logging (e.g., "input", "output", device name)
    identifier: String,
}

impl std::fmt::Debug for RubatoSRC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RubatoSRC")
            .field("identifier", &self.identifier)
            .field("input_rate", &self.input_rate)
            .field("output_rate", &self.output_rate)
            .field("ratio", &self.ratio)
            .field("input_frames", &self.input_frames)
            .field("output_frames", &self.output_frames)
            .finish()
    }
}

impl RubatoSRC {
    /// Create a Sinc-based fixed output resampler with dynamic ratio adjustment for clock sync
    /// This is ideal for real-time clock synchronization as the ratio can be adjusted on-the-fly
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz (e.g., 48000)
    /// * `output_rate` - Output sample rate in Hz (e.g., 44100)
    /// * `chunk_size_out` - Fixed number of output frames per call (e.g., 512)
    /// * `channels` - Number of audio channels (1 for mono, 2 for stereo)
    /// * `identifier` - Identifier for logging (e.g., "input", "output", device name)
    ///
    /// # Returns
    /// Sinc-based resampler with adjustable ratio for dynamic clock synchronization
    pub fn new_sinc_fixed_output(
        input_rate: f32,
        output_rate: f32,
        chunk_size_out: usize,
        channels: usize,
        identifier: String,
    ) -> Result<Self, String> {
        let resample_ratio = output_rate as f64 / input_rate as f64;

        info!(
            "🎯 {}: Creating SincFixedOut resampler {}Hz→{}Hz with {} output frames, {} channels (ratio: {:.6})",
            "SINC_FIXED_OUT".magenta(),
            input_rate,
            output_rate,
            chunk_size_out,
            channels,
            resample_ratio
        );

        // **DYNAMIC RATIO ADJUSTMENT**: Allow ±5% ratio adjustment for clock sync
        let max_resample_ratio_relative = 1.05; // Can adjust ratio ±5%

        // **HIGH QUALITY SINC PARAMETERS**: Balanced quality vs. performance
        let sinc_params = SincInterpolationParameters {
            sinc_len: 256,                                   // Good quality sinc length
            f_cutoff: 0.95,                                  // Anti-aliasing filter cutoff
            interpolation: SincInterpolationType::Linear,    // Linear interpolation (fastest)
            oversampling_factor: 160,                        // Reasonable oversampling for quality
            window: rubato::WindowFunction::BlackmanHarris2, // Good quality window
        };

        // Create Rubato's Sinc-based fixed output resampler
        let resampler = SincFixedOut::new(
            resample_ratio,              // Starting ratio
            max_resample_ratio_relative, // ±5% adjustment range
            sinc_params,                 // Interpolation parameters
            chunk_size_out,              // Fixed output chunk size in frames
            channels,                    // nbr_channels: dynamic (mono or stereo)
        )
        .map_err(|e| format!("Failed to create SincFixedOut resampler: {}", e))?;

        // Get the maximum possible input frame count from the resampler
        let max_input_frames = resampler.input_frames_max();

        info!(
            "🎯 {}: SincFixedOut configured: max {} input frames → {} output frames (±{:.1}% ratio adjust)",
            "SINC_FIXED_OUT".magenta(),
            max_input_frames,
            chunk_size_out,
            (max_resample_ratio_relative - 1.0) * 100.0
        );

        // Pre-allocate buffers for zero-allocation processing
        let input_buffer = vec![vec![0.0; max_input_frames]; channels]; // Maximum possible input size
        let output_buffer = vec![vec![0.0; chunk_size_out]; channels]; // Dynamic channel count

        info!(
            "📊 {}: Buffer allocation: input {}×{}, output {}×{} frames",
            "SINC_BUFFER_ALLOC".cyan(),
            max_input_frames,
            channels,
            chunk_size_out,
            channels
        );

        Ok(Self {
            resampler,
            input_buffer,
            output_buffer,
            input_rate,
            output_rate,
            ratio: output_rate / input_rate,
            input_frames: max_input_frames,
            output_frames: chunk_size_out,
            reusable_result_buffer: Vec::with_capacity(chunk_size_out * channels), // Dynamic channel samples
            identifier,
        })
    }

    pub fn input_rate(&self) -> u32 {
        self.input_rate as u32
    }

    pub fn output_rate(&self) -> u32 {
        self.output_rate as u32
    }

    /// Convert input samples to output sample rate using SincFixedOut
    /// Accepts variable input frame size and returns exactly the fixed output frames
    ///
    /// # Arguments
    /// * `input_samples` - Input audio samples at input_rate (interleaved stereo)
    ///
    /// # Returns
    /// Vector of resampled audio with fixed output frame size
    pub fn convert(&mut self, input_samples: &[f32]) -> Vec<f32> {
        // Handle empty input
        if input_samples.is_empty() {
            return Vec::new();
        }

        // Handle the conversion using SincFixedOut
        let channels = self.input_buffer.len();
        let input_frames = input_samples.len() / channels;

        // SincFixedOut requires exactly the number of frames it asks for
        let required_frames = self.resampler.input_frames_next();

        // Check if we have enough input frames
        if input_frames < required_frames {
            // info!(
            //     "⚠️ {}: [{}] Insufficient input frames: got {}, need {} - skipping conversion",
            //     "SINC_FIXED_OUT_SKIP".yellow(),
            //     self.identifier,
            //     input_frames,
            //     required_frames
            // );
            // return Vec::new();
        }

        // Use exactly the required number of frames
        let frames_to_process = required_frames;

        // Clear the input buffer up to required frames (dynamic channel count)
        for channel in 0..channels {
            for i in 0..frames_to_process {
                self.input_buffer[channel][i] = 0.0;
            }
        }

        // **DYNAMIC DE-INTERLEAVING**: Handle mono or stereo input
        for frame in 0..frames_to_process {
            for channel in 0..channels {
                let sample_index = frame * channels + channel;
                if sample_index < input_samples.len() {
                    self.input_buffer[channel][frame] = input_samples[sample_index];
                }
            }
        }

        // **DYNAMIC INPUT SLICES**: Prepare slices for all channels
        let mut input_slices: Vec<&[f32]> = Vec::with_capacity(channels);
        for channel in 0..channels {
            input_slices.push(&self.input_buffer[channel][..frames_to_process]);
        }

        // Perform resampling using SincFixedOut
        let process_result =
            self.resampler
                .process_into_buffer(&input_slices, &mut self.output_buffer, None);

        match process_result {
            Ok((input_frames_used, output_frames_generated)) => {
                // **CONSUMPTION VERIFICATION**: Check if Sinc consumed what we expected
                if input_frames_used != frames_to_process {
                    info!(
                        "🚨 {}: Consumption mismatch! Expected {}, actually used {} (deficit: {})",
                        "SINC_CONSUMPTION_ERROR".red(),
                        frames_to_process,
                        input_frames_used,
                        frames_to_process as i32 - input_frames_used as i32
                    );
                }

                // Log successful conversions with consumption details
                static SINC_CONVERSION_LOG_COUNT: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let conv_count =
                    SINC_CONVERSION_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                if conv_count < 10 || conv_count % 500 == 0 {
                    info!(
                        "🎯 {}: {} frames → {} frames, consumed {}/{} input",
                        "SINC_CONVERT".magenta(),
                        frames_to_process,
                        output_frames_generated,
                        input_frames_used,
                        frames_to_process
                    );
                }

                // SincFixedOut should always generate exactly the configured output frames
                // **DYNAMIC CHANNEL RE-INTERLEAVING**: Interleave all channels
                let channels = self.output_buffer.len();
                self.reusable_result_buffer.clear();
                self.reusable_result_buffer
                    .reserve(output_frames_generated * channels);

                for frame in 0..output_frames_generated {
                    for channel in 0..channels {
                        self.reusable_result_buffer
                            .push(self.output_buffer[channel][frame]);
                    }
                }

                self.reusable_result_buffer.clone()
            }
            Err(e) => {
                info!(
                    "❌ {}: Resampling failed: {}",
                    "SINC_FIXED_OUT_ERROR".red(),
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
        self.resampler.output_delay() as f32
    }

    /// Get the fixed input frame size (FixedInput) or max input frame size (FixedOutput)
    pub fn get_input_frames(&self) -> usize {
        self.input_frames
    }

    /// Get the fixed output frame size (FixedOutput) or max output frame size (FixedInput)
    pub fn get_output_frames(&self) -> usize {
        self.output_frames
    }

    /// Calculate number of input frames needed to produce desired output frames
    /// SincFixedOut tells us exactly how many frames it needs for the next call
    ///
    /// # Arguments
    /// * `desired_output_frames` - Number of output frames desired (unused for SincFixedOut)
    ///
    /// # Returns
    /// Number of input frames needed for the next processing call
    pub fn input_frames_needed(&mut self, _desired_output_frames: usize) -> usize {
        // SincFixedOut tells us exactly how many frames it needs next
        self.resampler.input_frames_next()
    }

    /// Dynamically adjust the sample rate ratio for clock synchronization
    /// SincFixedOut supports real-time ratio adjustment without recreation
    ///
    /// # Arguments
    /// * `new_input_rate` - New input sample rate
    /// * `new_output_rate` - New output sample rate
    /// * `ramp` - Whether to ramp the change (smoother but slower) or apply immediately
    ///
    /// # Returns
    /// Result indicating success or failure
    pub fn set_sample_rates(
        &mut self,
        new_input_rate: f32,
        new_output_rate: f32,
        ramp: bool,
        device_id: String,
    ) -> Result<(), String> {
        let new_ratio = new_output_rate as f64 / new_input_rate as f64;

        // **DYNAMIC RATIO ADJUSTMENT**: Update ratio without recreation
        self.resampler
            .set_resample_ratio(new_ratio, ramp)
            .map_err(|e| format!("Failed to set sample rate ratio {:.6}: {:?}", new_ratio, e))?;

        // Update our stored rates and ratio
        self.input_rate = new_input_rate;
        self.output_rate = new_output_rate;
        self.ratio = new_output_rate / new_input_rate;

        // Rate-limited logging for dynamic adjustments
        static DYNAMIC_ADJUST_COUNT: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let adjust_count = DYNAMIC_ADJUST_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let should_log = adjust_count % 1000 == 0;

        if should_log {
            info!(
                "🔄 {}: Dynamic ratio adjusted to {:.6} ({}Hz→{}Hz, ramp: {}) for device {}",
                "DYNAMIC_RATIO_ADJUST".green(),
                new_ratio,
                new_input_rate,
                new_output_rate,
                ramp,
                device_id
            );
        }

        Ok(())
    }

    /// Get the current resample ratio for monitoring
    pub fn get_current_ratio(&self) -> f64 {
        self.output_rate as f64 / self.input_rate as f64
    }
}
