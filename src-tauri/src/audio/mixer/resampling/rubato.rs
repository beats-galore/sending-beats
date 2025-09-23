use anyhow::Result;
use colored::*;
use rubato::{
    FftFixedIn, FftFixedOut, Resampler as RubatoResampler, SincFixedOut,
    SincInterpolationParameters, SincInterpolationType,
};
use samplerate_rs::{convert, ConverterType};
use tracing::info;
/// Sample Rate Converter for Dynamic Audio Buffer Conversion
///
/// Handles real-time sample rate conversion between input and output devices
/// Supports both upsampling (interpolation) and downsampling (decimation)
/// Optimized for low-latency audio processing in callback contexts

/// Resampler wrapper supporting FFT and Sinc variants with dynamic ratio adjustment
enum ResamplerWrapper {
    /// FFT-based fixed input resampler (variable output)
    FixedInput(FftFixedIn<f32>),
    /// FFT-based fixed output resampler (variable input)
    FixedOutput(FftFixedOut<f32>),
    /// Sinc-based fixed output resampler (variable input, adjustable ratio)
    SincFixedOutput(SincFixedOut<f32>),
}

impl std::fmt::Debug for ResamplerWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResamplerWrapper::FixedInput(_) => write!(f, "ResamplerWrapper(FftFixedIn)"),
            ResamplerWrapper::FixedOutput(_) => write!(f, "ResamplerWrapper(FftFixedOut)"),
            ResamplerWrapper::SincFixedOutput(_) => write!(f, "ResamplerWrapper(SincFixedOut)"),
        }
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
    /// Fixed input chunk size this resampler expects (FixedInput) or max input (FixedOutput)
    input_frames: usize,
    /// Fixed output chunk size this resampler expects (FixedOutput) or max output (FixedInput)
    output_frames: usize,
    /// **PERFORMANCE FIX**: Reusable result buffer to eliminate Vec allocations
    reusable_result_buffer: Vec<f32>,
}

impl RubatoSRC {
    /// Create a FFT-based fixed input resampler using FftFixedIn with calculated input chunk size
    /// Automatically calculates optimal input chunk size to produce approximately 1024 output samples
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz (e.g., 48000)
    /// * `output_rate` - Output sample rate in Hz (e.g., 44100)
    /// * `target_output_frames` - Desired output frames per call (e.g., 512 for 1024 samples)
    ///
    /// # Returns
    /// FFT-based resampler with calculated input chunk size for stable output
    pub fn new_fft_fixed_input(
        input_rate: f32,
        output_rate: f32,
        input_frames: usize,
    ) -> Result<Self, String> {
        // **CALCULATE OPTIMAL INPUT CHUNK SIZE**: Similar to R8brain approach
        let resampling_ratio = input_rate / output_rate;
        info!(
            "🧮 {}: Calculating input chunk for {}Hz→{}Hz input {} frames",
            "FFT_CHUNK_CALC".yellow(),
            input_rate,
            output_rate,
            input_frames
        );
        info!(
            "🧮 {}: Ratio {:.3} → raw {} frames",
            "FFT_CHUNK_CALC".yellow(),
            resampling_ratio,
            input_frames
        );

        // Create Rubato's FFT-based fixed input resampler
        let resampler = FftFixedIn::new(
            input_rate as usize,  // sample_rate_input
            output_rate as usize, // sample_rate_output
            input_frames,         // chunk_size_in: calculated input chunk size in frames
            4,                    // sub_chunks: desired number of subchunks for processing
            2,                    // nbr_channels: number of channels (stereo)
        )
        .map_err(|e| format!("Failed to create FftFixedIn resampler: {}", e))?;

        // Get the maximum possible output frame count from the resampler
        let max_output_frames = resampler.output_frames_max();

        info!(
            "🎯 {}: FftFixedIn configured: {} input frames → max {} output frames ( ratio: {:.3})",
            "FFT_FIXED_IN".blue(),
            input_frames,
            max_output_frames,
            output_rate / input_rate
        );

        // Pre-allocate buffers for zero-allocation processing
        let input_buffer = vec![vec![0.0; input_frames]; 2]; // 2 channels for stereo
        let output_buffer = vec![vec![0.0; max_output_frames]; 2]; // Maximum possible output size

        info!(
            "📊 {}: Buffer allocation: input {}×2, output {}×2 frames",
            "FFT_BUFFER_ALLOC".cyan(),
            input_frames,
            max_output_frames
        );

        Ok(Self {
            resampler: ResamplerWrapper::FixedInput(resampler),
            input_buffer,
            output_buffer,
            input_rate,
            output_rate,
            ratio: output_rate / input_rate,
            input_frames: input_frames,
            output_frames: max_output_frames,
            reusable_result_buffer: Vec::with_capacity(max_output_frames * 2), // Stereo samples
        })
    }

    /// Create a FFT-based fixed output resampler using FftFixedOut
    /// This accepts a variable number of input frames and returns a fixed number of output frames
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz (e.g., 48000)
    /// * `output_rate` - Output sample rate in Hz (e.g., 44100)
    /// * `chunk_size_out` - Fixed number of output frames per call (e.g., 1024)
    ///
    /// # Returns
    /// FFT-based resampler with variable input and fixed output frame sizes
    pub fn new_fft_fixed_output(
        input_rate: f32,
        output_rate: f32,
        chunk_size_out: usize,
    ) -> Result<Self, String> {
        info!(
            "🎯 {}: Creating FftFixedOut resampler {}Hz→{}Hz with {} output frames",
            "FFT_FIXED_OUT".green(),
            input_rate,
            output_rate,
            chunk_size_out
        );

        // Create Rubato's FFT-based fixed output resampler
        let resampler = FftFixedOut::new(
            input_rate as usize,  // sample_rate_input
            output_rate as usize, // sample_rate_output
            chunk_size_out,       // chunk_size_out: fixed output chunk size in frames
            4,                    // sub_chunks: desired number of subchunks for processing
            2,                    // nbr_channels: number of channels (stereo)
        )
        .map_err(|e| format!("Failed to create FftFixedOut resampler: {}", e))?;

        // Get the maximum possible input frame count from the resampler
        let max_input_frames = resampler.input_frames_max();

        info!(
            "🎯 {}: FftFixedOut configured: max {} input frames → {} output frames (ratio: {:.3})",
            "FFT_FIXED_OUT".green(),
            max_input_frames,
            chunk_size_out,
            output_rate / input_rate
        );

        // Pre-allocate buffers for zero-allocation processing
        let input_buffer = vec![vec![0.0; max_input_frames]; 2]; // Maximum possible input size
        let output_buffer = vec![vec![0.0; chunk_size_out]; 2]; // 2 channels for stereo

        Ok(Self {
            resampler: ResamplerWrapper::FixedOutput(resampler),
            input_buffer,
            output_buffer,
            input_rate,
            output_rate,
            ratio: output_rate / input_rate,
            input_frames: max_input_frames,
            output_frames: chunk_size_out,
            reusable_result_buffer: Vec::with_capacity(chunk_size_out * 2), // Stereo samples
        })
    }

    /// Create a Sinc-based fixed output resampler with dynamic ratio adjustment for clock sync
    /// This is ideal for real-time clock synchronization as the ratio can be adjusted on-the-fly
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz (e.g., 48000)
    /// * `output_rate` - Output sample rate in Hz (e.g., 44100)
    /// * `chunk_size_out` - Fixed number of output frames per call (e.g., 512)
    ///
    /// # Returns
    /// Sinc-based resampler with adjustable ratio for dynamic clock synchronization
    pub fn new_sinc_fixed_output(
        input_rate: f32,
        output_rate: f32,
        chunk_size_out: usize,
    ) -> Result<Self, String> {
        let resample_ratio = output_rate as f64 / input_rate as f64;

        info!(
            "🎯 {}: Creating SincFixedOut resampler {}Hz→{}Hz with {} output frames (ratio: {:.6})",
            "SINC_FIXED_OUT".magenta(),
            input_rate,
            output_rate,
            chunk_size_out,
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
            2,                           // nbr_channels: stereo
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
        let input_buffer = vec![vec![0.0; max_input_frames]; 2]; // Maximum possible input size
        let output_buffer = vec![vec![0.0; chunk_size_out]; 2]; // 2 channels for stereo

        info!(
            "📊 {}: Buffer allocation: input {}×2, output {}×2 frames",
            "SINC_BUFFER_ALLOC".cyan(),
            max_input_frames,
            chunk_size_out
        );

        Ok(Self {
            resampler: ResamplerWrapper::SincFixedOutput(resampler),
            input_buffer,
            output_buffer,
            input_rate,
            output_rate,
            ratio: output_rate / input_rate,
            input_frames: max_input_frames,
            output_frames: chunk_size_out,
            reusable_result_buffer: Vec::with_capacity(chunk_size_out * 2), // Stereo samples
        })
    }

    pub fn input_rate(&self) -> u32 {
        self.input_rate as u32
    }

    pub fn output_rate(&self) -> u32 {
        self.output_rate as u32
    }

    /// Convert input samples to output sample rate using either FftFixedIn or FftFixedOut
    /// Behavior depends on the resampler type:
    /// - FftFixedIn: Accepts exactly the fixed input frame size and returns variable output frames
    /// - FftFixedOut: Accepts variable input frame size and returns exactly the fixed output frames
    ///
    /// # Arguments
    /// * `input_samples` - Input audio samples at input_rate (interleaved stereo)
    ///
    /// # Returns
    /// Vector of resampled audio (length depends on resampler type)
    pub fn convert(&mut self, input_samples: &[f32]) -> Vec<f32> {
        // Handle empty input
        if input_samples.is_empty() {
            return Vec::new();
        }

        // Handle the conversion differently based on resampler type
        match &mut self.resampler {
            ResamplerWrapper::FixedInput(resampler) => {
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
                        self.input_buffer[1][frame] = input_samples[frame * 2 + 1];
                        // Right channel
                    }
                }

                // Perform resampling using FftFixedIn
                let process_result = resampler.process_into_buffer(
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
            ResamplerWrapper::FixedOutput(resampler) => {
                let input_frames = input_samples.len() / 2; // Each frame has L+R samples

                // FftFixedOut requires exactly the number of frames it asks for
                let required_frames = resampler.input_frames_next();

                // Check if we have enough input frames
                if input_frames < required_frames {
                    info!(
                        "⚠️ {}: Insufficient input frames: got {}, need {} - skipping conversion",
                        "FFT_FIXED_OUT_SKIP".yellow(),
                        input_frames,
                        required_frames
                    );
                    return Vec::new();
                }

                // Use exactly the required number of frames
                let frames_to_process = required_frames;

                // Clear the input buffer up to required frames
                for i in 0..frames_to_process {
                    self.input_buffer[0][i] = 0.0;
                    self.input_buffer[1][i] = 0.0;
                }

                // De-interleave input samples: LRLRLR... -> L...L, R...R
                // Use exactly the required frames
                for frame in 0..frames_to_process {
                    if frame * 2 + 1 < input_samples.len() {
                        self.input_buffer[0][frame] = input_samples[frame * 2]; // Left channel
                        self.input_buffer[1][frame] = input_samples[frame * 2 + 1];
                        // Right channel
                    }
                }

                // Prepare input slices for exactly the required frames
                let input_slices = [
                    &self.input_buffer[0][..frames_to_process],
                    &self.input_buffer[1][..frames_to_process],
                ];

                // Perform resampling using FftFixedOut
                let process_result =
                    resampler.process_into_buffer(&input_slices, &mut self.output_buffer, None);

                match process_result {
                    Ok((input_frames_used, output_frames_generated)) => {
                        // **CONSUMPTION VERIFICATION**: Check if Rubato consumed what we expected
                        if input_frames_used != frames_to_process {
                            info!(
                                "🚨 {}: Consumption mismatch! Expected {}, actually used {} (deficit: {})",
                                "RUBATO_CONSUMPTION_ERROR".red(),
                                frames_to_process,
                                input_frames_used,
                                frames_to_process as i32 - input_frames_used as i32
                            );
                        }

                        // Log successful conversions with consumption details
                        static CONVERSION_LOG_COUNT: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let conv_count =
                            CONVERSION_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                        if conv_count < 10 || conv_count % 500 == 0 {
                            info!(
                                "🎯 {}: {} frames → {} frames, consumed {}/{} input",
                                "RUBATO_CONVERT".cyan(),
                                frames_to_process,
                                output_frames_generated,
                                input_frames_used,
                                frames_to_process
                            );
                        }

                        // FftFixedOut should always generate exactly the configured output frames
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
                            "FFT_FIXED_OUT_ERROR".red(),
                            e
                        );
                        Vec::new()
                    }
                }
            }
            ResamplerWrapper::SincFixedOutput(resampler) => {
                let input_frames = input_samples.len() / 2; // Each frame has L+R samples

                // SincFixedOut requires exactly the number of frames it asks for
                let required_frames = resampler.input_frames_next();

                // Check if we have enough input frames
                if input_frames < required_frames {
                    info!(
                        "⚠️ {}: Insufficient input frames: got {}, need {} - skipping conversion",
                        "SINC_FIXED_OUT_SKIP".yellow(),
                        input_frames,
                        required_frames
                    );
                    return Vec::new();
                }

                // Use exactly the required number of frames
                let frames_to_process = required_frames;

                // Clear the input buffer up to required frames
                for i in 0..frames_to_process {
                    self.input_buffer[0][i] = 0.0;
                    self.input_buffer[1][i] = 0.0;
                }

                // De-interleave input samples: LRLRLR... -> L...L, R...R
                // Use exactly the required frames
                for frame in 0..frames_to_process {
                    if frame * 2 + 1 < input_samples.len() {
                        self.input_buffer[0][frame] = input_samples[frame * 2]; // Left channel
                        self.input_buffer[1][frame] = input_samples[frame * 2 + 1];
                        // Right channel
                    }
                }

                // Prepare input slices for exactly the required frames
                let input_slices = [
                    &self.input_buffer[0][..frames_to_process],
                    &self.input_buffer[1][..frames_to_process],
                ];

                // Perform resampling using SincFixedOut
                let process_result =
                    resampler.process_into_buffer(&input_slices, &mut self.output_buffer, None);

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
                        let conv_count = SINC_CONVERSION_LOG_COUNT
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

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
                            "SINC_FIXED_OUT_ERROR".red(),
                            e
                        );
                        Vec::new()
                    }
                }
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
        match &self.resampler {
            ResamplerWrapper::FixedInput(resampler) => resampler.output_delay() as f32,
            ResamplerWrapper::FixedOutput(resampler) => resampler.output_delay() as f32,
            ResamplerWrapper::SincFixedOutput(resampler) => resampler.output_delay() as f32,
        }
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
    /// This provides consistent API across all resampler implementations
    ///
    /// # Arguments
    /// * `desired_output_frames` - Number of output frames desired
    ///
    /// # Returns
    /// Number of input frames needed (uses rubato's input_frames_next for accuracy)
    pub fn input_frames_needed(&mut self, _desired_output_frames: usize) -> usize {
        // For rubato, use the built-in input_frames_next method
        // This gives the exact number of frames needed for the next processing call
        match &mut self.resampler {
            ResamplerWrapper::FixedInput(_resampler) => {
                // FftFixedIn always needs the same fixed input size
                self.input_frames
            }
            ResamplerWrapper::FixedOutput(resampler) => {
                // FftFixedOut tells us exactly how many frames it needs next
                resampler.input_frames_next()
            }
            ResamplerWrapper::SincFixedOutput(resampler) => {
                // SincFixedOut tells us exactly how many frames it needs next
                resampler.input_frames_next()
            }
        }
    }

    /// Check if this resampler supports dynamic sample rate adjustment without recreation
    /// This is crucial for real-time clock synchronization
    pub fn supports_dynamic_sample_rate(&self) -> bool {
        match &self.resampler {
            ResamplerWrapper::FixedInput(_) => false, // FftFixedIn: No dynamic ratio support
            ResamplerWrapper::FixedOutput(_) => false, // FftFixedOut: No dynamic ratio support
            ResamplerWrapper::SincFixedOutput(_) => true, // SincFixedOut: Full dynamic ratio support
        }
    }

    /// Dynamically adjust the sample rate ratio for clock synchronization
    /// Only works with SincFixedOut - other resamplers will return an error
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
    ) -> Result<(), String> {
        let new_ratio = new_output_rate as f64 / new_input_rate as f64;

        match &mut self.resampler {
            ResamplerWrapper::SincFixedOutput(resampler) => {
                // **DYNAMIC RATIO ADJUSTMENT**: Update ratio without recreation
                resampler.set_resample_ratio(new_ratio, ramp).map_err(|e| {
                    format!("Failed to set sample rate ratio {:.6}: {:?}", new_ratio, e)
                })?;

                // Update our stored rates and ratio
                self.input_rate = new_input_rate;
                self.output_rate = new_output_rate;
                self.ratio = new_output_rate / new_input_rate;

                info!(
                    "🔄 {}: Dynamic ratio adjusted to {:.6} ({}Hz→{}Hz, ramp: {})",
                    "DYNAMIC_RATIO_ADJUST".green(),
                    new_ratio,
                    new_input_rate,
                    new_output_rate,
                    ramp
                );

                Ok(())
            }
            _ => Err(
                "Dynamic sample rate adjustment not supported by this resampler type".to_string(),
            ),
        }
    }

    /// Get the current resample ratio for monitoring
    pub fn get_current_ratio(&self) -> f64 {
        self.output_rate as f64 / self.input_rate as f64
    }
}
