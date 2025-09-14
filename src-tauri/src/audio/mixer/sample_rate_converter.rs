use anyhow::Result;
use r8brain_rs::{PrecisionProfile, Resampler as R8BrainResampler};
use rubato::{
    FastFixedIn, PolynomialDegree, Resampler as RubatoResampler, SincFixedIn,
    SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
/// Sample Rate Converter for Dynamic Audio Buffer Conversion
///
/// Handles real-time sample rate conversion between input and output devices
/// Supports both upsampling (interpolation) and downsampling (decimation)
/// Optimized for low-latency audio processing in callback contexts
use std::collections::VecDeque;

/// Resampler type enum for supporting both high-quality and fast resampling
enum ResamplerType {
    /// High-quality sinc interpolation (broadcast quality, higher CPU)
    HighQuality(SincFixedIn<f32>),
    /// Fast fixed interpolation (lower quality, much lower CPU)
    Fast(FastFixedIn<f32>),
}

/// Professional sample rate converter using Rubato's windowed sinc interpolation
/// Provides broadcast-quality, transparent audio resampling with anti-aliasing
/// Optimized for real-time audio processing with pre-allocated buffers
pub struct RubatoSRC {
    /// Rubato resampler (high-quality or fast)
    resampler: ResamplerType,
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
    /// Buffer accumulator to smooth output size variations
    pub accumulator: Vec<f32>,
    /// Target output chunk size for consistent delivery
    pub target_output_chunk_size: usize,
    /// **PERFORMANCE FIX**: Reusable result buffer to eliminate Vec allocations
    reusable_result_buffer: Vec<f32>,
}

impl RubatoSRC {
    /// Create a new professional sample rate converter with broadcast quality
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz (e.g., 44100)
    /// * `output_rate` - Output sample rate in Hz (e.g., 48000)
    ///
    /// # Returns
    /// High-quality resampler with windowed sinc interpolation and anti-aliasing
    pub fn new(input_rate: f32, output_rate: f32) -> Result<Self, String> {
        Self::with_target_chunk_size(input_rate, output_rate, None)
    }

    /// Create a new FAST sample rate converter optimized for real-time performance
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz (e.g., 44100)
    /// * `output_rate` - Output sample rate in Hz (e.g., 48000)
    ///
    /// # Returns
    /// Fast resampler with lower quality but much lower CPU usage
    pub fn new_fast(input_rate: f32, output_rate: f32) -> Result<Self, String> {
        Self::with_target_chunk_size_fast(input_rate, output_rate, None)
    }

    /// Create a new resampler with a specific target output chunk size
    ///
    /// # Arguments
    /// * `target_chunk_size` - Desired output chunk size (should be power of 2), None for auto-detection
    pub fn with_target_chunk_size(
        input_rate: f32,
        output_rate: f32,
        target_chunk_size: Option<usize>,
    ) -> Result<Self, String> {
        // Calculate appropriate max chunk size based on sample rates
        // Higher sample rates might need larger buffers for efficiency
        let max_rate = input_rate.max(output_rate);
        let suggested_max_frames = if max_rate > 96000.0 {
            4096 // High sample rates (>96kHz) - larger chunks for efficiency
        } else if max_rate > 48000.0 {
            2048 // Standard high quality (48-96kHz)
        } else {
            1024 // Standard quality (<=48kHz)
        };

        println!(
            "🔧 RUBATO_INIT: Creating resampler {}Hz→{}Hz with max {} frames",
            input_rate, output_rate, suggested_max_frames
        );

        Self::with_max_frames(input_rate, output_rate, suggested_max_frames)
    }

    /// Create a new FAST resampler with a specific target output chunk size
    ///
    /// # Arguments
    /// * `target_chunk_size` - Desired output chunk size (should be power of 2), None for auto-detection
    pub fn with_target_chunk_size_fast(
        input_rate: f32,
        output_rate: f32,
        target_chunk_size: Option<usize>,
    ) -> Result<Self, String> {
        // Use smaller buffer sizes for fast resampler (lower latency)
        let max_rate = input_rate.max(output_rate);
        let suggested_max_frames = if max_rate > 96000.0 {
            2048 // High sample rates - smaller chunks for speed
        } else if max_rate > 48000.0 {
            1024 // Standard high quality
        } else {
            512 // Standard quality - smaller for speed
        };

        println!(
            "🔧 RUBATO_FAST: Creating FAST resampler {}Hz→{}Hz with max {} frames",
            input_rate, output_rate, suggested_max_frames
        );

        Self::with_max_frames_fast(input_rate, output_rate, suggested_max_frames)
    }

    /// Create a low-artifact resampler for testing (reduced quality but fewer artifacts)
    /// Use this temporarily to isolate if filter complexity is causing artifacts
    pub fn new_low_artifact(input_rate: f32, output_rate: f32) -> Result<Self, String> {
        let max_rate = input_rate.max(output_rate);
        let suggested_max_frames = if max_rate > 96000.0 {
            4096
        } else if max_rate > 48000.0 {
            2048
        } else {
            1024
        };

        println!(
            "🔧 RUBATO_LOW_ARTIFACT: Creating reduced-artifact resampler {}Hz→{}Hz",
            input_rate, output_rate
        );

        Self::with_target_chunk_size_and_params(
            input_rate,
            output_rate,
            None,
            suggested_max_frames,
            true,
        )
    }

    /// Create a new resampler with specified maximum input frame size
    pub fn with_max_frames(
        input_rate: f32,
        output_rate: f32,
        max_input_frames: usize,
    ) -> Result<Self, String> {
        Self::with_target_chunk_size_and_params(
            input_rate,
            output_rate,
            None,
            max_input_frames,
            false,
        )
    }

    /// Create a new FAST resampler with specified maximum input frame size
    pub fn with_max_frames_fast(
        input_rate: f32,
        output_rate: f32,
        max_input_frames: usize,
    ) -> Result<Self, String> {
        Self::with_target_chunk_size_and_params_fast(
            input_rate,
            output_rate,
            None,
            max_input_frames,
        )
    }

    /// Internal method with all parameters
    pub fn with_target_chunk_size_and_params(
        input_rate: f32,
        output_rate: f32,
        target_chunk_size: Option<usize>,
        max_input_frames: usize,
        low_artifact: bool,
    ) -> Result<Self, String> {
        Self::with_params_internal(
            input_rate,
            output_rate,
            target_chunk_size,
            max_input_frames,
            low_artifact,
        )
    }

    /// Internal method with all parameters for FAST resampling
    pub fn with_target_chunk_size_and_params_fast(
        input_rate: f32,
        output_rate: f32,
        target_chunk_size: Option<usize>,
        max_input_frames: usize,
    ) -> Result<Self, String> {
        Self::with_params_internal_fast(
            input_rate,
            output_rate,
            target_chunk_size,
            max_input_frames,
        )
    }

    /// Create a new resampler with configurable quality parameters
    pub fn with_params(
        input_rate: f32,
        output_rate: f32,
        max_input_frames: usize,
        low_artifact: bool,
    ) -> Result<Self, String> {
        Self::with_params_internal(input_rate, output_rate, None, max_input_frames, true)
    }

    /// Internal implementation with all parameters for FAST resampling
    fn with_params_internal_fast(
        input_rate: f32,
        output_rate: f32,
        target_chunk_size: Option<usize>,
        max_input_frames: usize,
    ) -> Result<Self, String> {
        println!("🚀 RUBATO_FAST: Creating FastFixedIn resampler (low CPU, lower quality)");

        // Create Rubato's fast fixed-input-size resampler
        // FastFixedIn::new(resample_ratio, max_ratio_change, polynomial_degree, num_channels, chunk_size)
        let resampler = FastFixedIn::new(
            output_rate as f64 / input_rate as f64, // resample_ratio
            2.0,                      // max_ratio_change (for future dynamic adjustment)
            PolynomialDegree::Septic, // polynomial_degree (Septic = 7th order for balance of speed/quality)
            max_input_frames,         // num_channels (stereo)
            2,                        // chunk_size (fixed input chunk size)
        )
        .map_err(|e| format!("Failed to create FastFixedIn resampler: {}", e))?;

        // Pre-allocate buffers for zero-allocation processing
        let input_buffer = vec![vec![0.0; max_input_frames]; 2]; // 2 channels for stereo
        let max_output_frames = ((max_input_frames as f64 * output_rate as f64 / input_rate as f64)
            .ceil() as usize)
            + 64; // Extra headroom
        let output_buffer = vec![vec![0.0; max_output_frames]; 2]; // 2 channels for stereo

        // Determine target output chunk size
        let target_output_chunk_size = if let Some(size) = target_chunk_size {
            // Use specified chunk size (should be power of 2)
            println!(
                "🔧 BUFFER_ACCUMULATOR_FAST: Using specified target chunk size: {} frames",
                size
            );
            size
        } else {
            // Smart power-of-2 size selection based on up/downsampling
            let calculated_size = (512.0 * output_rate as f64 / input_rate as f64).round() as usize;
            let is_downsampling = output_rate < input_rate;

            let power_of_2_size = if is_downsampling {
                // DOWNSAMPLING: Choose smaller power-of-2 to ensure samples are always available
                if calculated_size <= 128 {
                    128
                } else if calculated_size <= 256 {
                    256 // 48kHz→44.1kHz: 470 → choose 256 for guaranteed availability
                } else if calculated_size <= 512 {
                    512 // Still choose smaller to avoid waiting
                } else {
                    1024
                }
            } else {
                // UPSAMPLING: Choose larger power-of-2 as normal
                if calculated_size <= 256 {
                    256
                } else if calculated_size <= 512 {
                    512
                } else if calculated_size <= 1024 {
                    1024
                } else {
                    2048
                }
            };

            let strategy = if is_downsampling {
                "DOWNSAMPLING (smaller target)"
            } else {
                "UPSAMPLING (normal target)"
            };
            println!(
                "🔧 BUFFER_ACCUMULATOR_FAST: {} - Calculated {} frames, using power-of-2: {} frames",
                strategy, calculated_size, power_of_2_size
            );
            power_of_2_size
        };

        Ok(Self {
            resampler: ResamplerType::Fast(resampler),
            input_buffer,
            output_buffer,
            input_rate,
            output_rate,
            ratio: output_rate / input_rate,
            max_input_frames,
            accumulator: Vec::new(),
            target_output_chunk_size,
            reusable_result_buffer: Vec::with_capacity(8192), // Pre-allocate for reuse
        })
    }

    /// Internal implementation with all parameters
    fn with_params_internal(
        input_rate: f32,
        output_rate: f32,
        target_chunk_size: Option<usize>,
        max_input_frames: usize,
        low_artifact: bool,
    ) -> Result<Self, String> {
        // Configure sinc interpolation parameters - high quality vs low artifact
        let params = if low_artifact {
            println!("🔧 Using LOW ARTIFACT settings (may reduce quality)");
            SincInterpolationParameters {
                sinc_len: 64,                                 // Shorter filter = fewer artifacts
                f_cutoff: 0.9,                                // Less aggressive cutoff
                interpolation: SincInterpolationType::Linear, // Linear interpolation
                oversampling_factor: 64,                      // Lower oversampling
                window: WindowFunction::Hann,                 // Simpler window function
            }
        } else {
            println!("🔧 Using HIGH QUALITY settings (may have more artifacts)");
            SincInterpolationParameters {
                sinc_len: 256,                                // High-quality sinc filter length
                f_cutoff: 0.95, // Conservative cutoff to prevent aliasing
                interpolation: SincInterpolationType::Linear, // Linear interpolation between sinc samples
                oversampling_factor: 256,                     // High oversampling for quality
                window: WindowFunction::BlackmanHarris2,      // Excellent side-lobe suppression
            }
        };

        // Create Rubato's fixed-input-size resampler
        let resampler = SincFixedIn::new(
            output_rate as f64 / input_rate as f64,
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

        // Determine target output chunk size
        let target_output_chunk_size = if let Some(size) = target_chunk_size {
            // Use specified chunk size (should be power of 2)
            println!(
                "🔧 BUFFER_ACCUMULATOR: Using specified target chunk size: {} frames",
                size
            );
            size
        } else {
            // Smart power-of-2 size selection based on up/downsampling
            let calculated_size = (512.0 * output_rate as f64 / input_rate as f64).round() as usize;
            let is_downsampling = output_rate < input_rate;

            let power_of_2_size = if is_downsampling {
                // DOWNSAMPLING: Choose smaller power-of-2 to ensure samples are always available
                if calculated_size <= 128 {
                    128
                } else if calculated_size <= 256 {
                    256 // 48kHz→44.1kHz: 470 → choose 256 for guaranteed availability
                } else if calculated_size <= 512 {
                    512 // Still choose smaller to avoid waiting
                } else {
                    1024
                }
            } else {
                // UPSAMPLING: Choose larger power-of-2 as normal
                if calculated_size <= 256 {
                    256
                } else if calculated_size <= 512 {
                    512
                } else if calculated_size <= 1024 {
                    1024
                } else {
                    2048
                }
            };

            let strategy = if is_downsampling {
                "DOWNSAMPLING (smaller target)"
            } else {
                "UPSAMPLING (normal target)"
            };
            println!(
                "🔧 BUFFER_ACCUMULATOR: {} - Calculated {} frames, using power-of-2: {} frames",
                strategy, calculated_size, power_of_2_size
            );
            power_of_2_size
        };

        Ok(Self {
            resampler: ResamplerType::HighQuality(resampler),
            input_buffer,
            output_buffer,
            input_rate,
            output_rate,
            ratio: output_rate / input_rate,
            max_input_frames,
            accumulator: Vec::new(),
            target_output_chunk_size,
            reusable_result_buffer: Vec::with_capacity(8192), // Pre-allocate for reuse
        })
    }

    pub fn update_resampler_rate(&mut self, new_input_rate: u32) -> Result<()> {
        let ratio = self.output_rate as f64 / new_input_rate as f64;
        match &mut self.resampler {
            ResamplerType::HighQuality(resampler) => {
                resampler.set_resample_ratio(ratio, true);
            }
            ResamplerType::Fast(resampler) => {
                resampler.set_resample_ratio(ratio, true);
            }
        }
        Ok(())
    }

    /// Convert input samples to output sample rate with broadcast quality
    /// Uses dynamic output buffer sizing - no fixed output size calculation needed
    ///
    /// # Arguments
    /// * `input_samples` - Input audio samples at input_rate (interleaved stereo)
    ///
    /// # Returns
    /// Vector of resampled audio with actual dynamic length determined by rubato
    pub fn convert(&mut self, input_samples: &[f32]) -> Vec<f32> {
        // Entry log to verify function is called
        static CALL_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let count = CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if count < 5 {
            println!(
                "🚨 RUBATO_ENTRY: convert() called with {} samples (call #{})",
                input_samples.len(),
                count + 1
            );
        }

        // Handle empty input
        if input_samples.is_empty() {
            return Vec::new();
        }

        // Convert interleaved stereo to de-interleaved format for Rubato
        let input_frames = input_samples.len() / 2; // Each frame has L+R samples
        let input_frames = input_frames.min(self.max_input_frames);

        // For FastFixedIn, ensure we use exactly the chunk size it was configured with
        let input_frames = match &self.resampler {
            ResamplerType::HighQuality(_) => input_frames, // SincFixedIn supports variable sizes
            ResamplerType::Fast(_) => {
                // FastFixedIn requires exactly max_input_frames - pad if needed, truncate if too large
                if input_frames > self.max_input_frames {
                    println!(
                        "⚠️ FAST_RESAMPLER: Truncating {} frames to max {}",
                        input_frames, self.max_input_frames
                    );
                    self.max_input_frames
                } else if input_frames < self.max_input_frames {
                    // For FastFixedIn, we need to pad smaller inputs to the fixed size
                    // println!("🔧 FAST_RESAMPLER: Padding {} frames to fixed size {}", input_frames, self.max_input_frames);
                    input_frames // We'll handle padding in the buffer filling logic
                } else {
                    input_frames
                }
            }
        };

        if input_frames == 0 {
            return Vec::new();
        }

        // CONTINUITY DEBUG: Check for buffer size changes that could cause discontinuities
        static LAST_INPUT_SIZE: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        static LAST_OUTPUT_SIZE: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        let prev_input_size = LAST_INPUT_SIZE.load(std::sync::atomic::Ordering::Relaxed);
        // if prev_input_size != 0 && prev_input_size != input_frames {
        //     println!("⚠️ CONTINUITY_WARNING: Input size changed {} → {} frames (discontinuity risk!)",
        //              prev_input_size, input_frames);
        // }
        LAST_INPUT_SIZE.store(input_frames, std::sync::atomic::Ordering::Relaxed);

        // Set dynamic chunk size for this call - only SincFixedIn supports variable input sizes!
        match &mut self.resampler {
            ResamplerType::HighQuality(resampler) => {
                if let Err(e) = resampler.set_chunk_size(input_frames) {
                    println!(
                        "❌ RUBATO_CHUNK_SIZE_ERROR: Failed to set chunk size to {}: {}",
                        input_frames, e
                    );
                    return Vec::new();
                }
            }
            ResamplerType::Fast(_resampler) => {
                // FastFixedIn doesn't support dynamic chunk sizes - it uses the fixed size set during creation
                // Just ensure input_frames doesn't exceed max_input_frames
                if input_frames > self.max_input_frames {
                    println!(
                        "⚠️ FAST_RESAMPLER: Input size {} exceeds max {}, truncating",
                        input_frames, self.max_input_frames
                    );
                    // We'll handle this by limiting the input size below
                }
            }
        }

        // BUFFER RECREATION DEBUG: Check if we're constantly resizing (bad for performance and continuity)
        static RESIZE_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        static LAST_BUFFER_SIZE: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        let prev_buffer_size = LAST_BUFFER_SIZE.load(std::sync::atomic::Ordering::Relaxed);

        // **PERFORMANCE FIX**: Use pre-allocated buffers instead of constant resizing
        const MAX_FRAMES: usize = 2048; // Pre-allocate for maximum expected size

        // Ensure buffers are large enough (one-time allocation)
        if self.input_buffer[0].len() < MAX_FRAMES {
            self.input_buffer[0].resize(MAX_FRAMES, 0.0);
            self.input_buffer[1].resize(MAX_FRAMES, 0.0);
            println!(
                "📋 BUFFER_INIT: Pre-allocated {} frame buffers (eliminates resize overhead)",
                MAX_FRAMES
            );
        }

        // For FastFixedIn, we need to fill exactly max_input_frames (pad with zeros if needed)
        let frames_to_fill = match &self.resampler {
            ResamplerType::HighQuality(_) => input_frames, // Only fill what we have
            ResamplerType::Fast(_) => self.max_input_frames, // Always fill the fixed size
        };

        // Clear the buffer portion we'll use
        for i in 0..frames_to_fill {
            self.input_buffer[0][i] = 0.0;
            self.input_buffer[1][i] = 0.0;
        }

        // AUDIO QUALITY DEBUG: Check for input anomalies that could cause artifacts
        let input_peak = input_samples
            .iter()
            .map(|&s| s.abs())
            .fold(0.0f32, f32::max);
        let input_rms =
            (input_samples.iter().map(|&s| s * s).sum::<f32>() / input_samples.len() as f32).sqrt();
        static EXTREME_INPUT_COUNT: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);

        if input_peak > 0.99 || input_rms > 0.7 {
            let count = EXTREME_INPUT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if count < 5 {
                println!("⚠️ AUDIO_QUALITY: High input levels - peak: {:.3}, rms: {:.3} (clipping risk!)",
                         input_peak, input_rms);
            }
        }

        // De-interleave: LRLRLR... -> L...L, R...R
        // Fill actual data up to input_frames, pad with zeros if needed for FastFixedIn
        for frame in 0..frames_to_fill {
            if frame < input_frames && frame * 2 + 1 < input_samples.len() {
                self.input_buffer[0][frame] = input_samples[frame * 2]; // Left channel
                self.input_buffer[1][frame] = input_samples[frame * 2 + 1]; // Right channel
            }
            // else: already cleared to 0.0 above (padding for FastFixedIn)
        }

        // **PERFORMANCE FIX**: Pre-allocate output buffers to eliminate resize overhead
        // Use frames_to_fill for FastFixedIn (which includes padding) or input_frames for SincFixedIn
        let processing_frames = match &self.resampler {
            ResamplerType::HighQuality(_) => input_frames,
            ResamplerType::Fast(_) => frames_to_fill, // Use the padded size for output calculation
        };
        let max_output_frames = ((processing_frames as f64 * self.output_rate as f64
            / self.input_rate as f64)
            .ceil() as usize)
            + 64;
        const MAX_OUTPUT_FRAMES: usize = 4096; // Pre-allocate for maximum expected output

        // Ensure output buffers are large enough (one-time allocation)
        if self.output_buffer[0].len() < MAX_OUTPUT_FRAMES {
            self.output_buffer[0].resize(MAX_OUTPUT_FRAMES, 0.0);
            self.output_buffer[1].resize(MAX_OUTPUT_FRAMES, 0.0);
        }

        // Clear only the portion we'll use
        for i in 0..max_output_frames.min(MAX_OUTPUT_FRAMES) {
            self.output_buffer[0][i] = 0.0;
            self.output_buffer[1][i] = 0.0;
        }

        // Perform resampling with dynamic output sizing (high-quality or fast)
        let process_result = match &mut self.resampler {
            ResamplerType::HighQuality(resampler) => {
                resampler.process_into_buffer(&self.input_buffer, &mut self.output_buffer, None)
            }
            ResamplerType::Fast(resampler) => {
                resampler.process_into_buffer(&self.input_buffer, &mut self.output_buffer, None)
            }
        };

        match process_result {
            Ok((_input_frames_used, output_frames_generated)) => {
                // CONTINUITY DEBUG: Check for output size variations
                let prev_output_size = LAST_OUTPUT_SIZE.load(std::sync::atomic::Ordering::Relaxed);

                LAST_OUTPUT_SIZE.store(
                    output_frames_generated,
                    std::sync::atomic::Ordering::Relaxed,
                );

                // RATIO DEBUG: Check if resampling ratio matches expected
                let expected_output_frames = (input_frames as f64 * self.output_rate as f64
                    / self.input_rate as f64)
                    .round() as usize;
                let ratio_error =
                    (output_frames_generated as f64 - expected_output_frames as f64).abs();
                if ratio_error > 2.0 {
                    println!(
                        "⚠️ RATIO_MISMATCH: Expected ~{} output frames, got {} (error: {:.1})",
                        expected_output_frames, output_frames_generated, ratio_error
                    );
                }

                // **PERFORMANCE FIX**: Use reusable buffer to eliminate Vec allocation on every convert() call
                self.reusable_result_buffer.clear();
                self.reusable_result_buffer.reserve(output_frames_generated * 2);
                for frame in 0..output_frames_generated {
                    self.reusable_result_buffer.push(self.output_buffer[0][frame]); // Left
                    self.reusable_result_buffer.push(self.output_buffer[1][frame]); // Right
                }
                let result = self.reusable_result_buffer.clone(); // Final unavoidable clone for API

                static DIRECT_OUTPUT_COUNT: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let count = DIRECT_OUTPUT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if count < 5 || count % 1000 == 0 {
                    println!(
                        "🎯 DIRECT_OUTPUT: Returning ALL {} resampled samples immediately (no accumulator delay, call #{})",
                        result.len(), count
                    );
                }

                // ARTIFACT DEBUG: Only check for clipping artifacts
                if !result.is_empty() {
                    let output_peak = result.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let output_rms =
                        (result.iter().map(|&s| s * s).sum::<f32>() / result.len() as f32).sqrt();

                    // Only flag actual clipping (>1.0) - gain comparison is invalid with accumulator
                    static CLIPPING_COUNT: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(0);
                    if output_peak > 1.0 {
                        let count =
                            CLIPPING_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if count < 10 {
                            println!(
                                "🚨 CLIPPING_DETECTED: Output peak {:.3} > 1.0 (digital clipping!)",
                                output_peak
                            );
                        }
                    }

                    // Rate-limited DEBUG: Track output conversion (every 1000 calls)
                    use std::sync::{LazyLock, Mutex as StdMutex};
                    static RUBATO_DEBUG_COUNT: LazyLock<StdMutex<u64>> =
                        LazyLock::new(|| StdMutex::new(0));
                    if let Ok(mut count) = RUBATO_DEBUG_COUNT.lock() {
                        *count += 1;
                        if *count <= 3 || *count % 1000 == 0 {
                            println!(
                                "🔍 RUBATO_SMOOTH: {} input → {} consistent output frames, peak: {:.3}→{:.3} (call #{})",
                                input_frames,
                                result.len() / 2,
                                input_peak, output_peak,
                                count
                            );
                        }
                    }
                }

                result
            }
            Err(e) => {
                println!("❌ RUBATO_ERROR: Resampling failed: {}", e);
                // Return empty vector on error - no fixed size assumption
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
        match &self.resampler {
            ResamplerType::HighQuality(resampler) => resampler.output_delay() as f32,
            ResamplerType::Fast(resampler) => resampler.output_delay() as f32,
        }
    }

    /// Get the target chunk size for consistent output delivery
    pub fn get_target_chunk_size(&self) -> usize {
        self.target_output_chunk_size
    }

    /// Get the current number of samples in the accumulator buffer
    pub fn get_accumulator_size(&self) -> usize {
        self.accumulator.len()
    }

    /// Drain samples from accumulator without processing new input
    /// Used when accumulator has enough samples and we want to avoid overflow
    pub fn drain_accumulator_only(&mut self) -> Vec<f32> {
        let target_samples = self.target_output_chunk_size * 2; // Stereo samples
        if self.accumulator.len() >= target_samples {
            // **PERFORMANCE FIX**: Use reusable buffer instead of drain().collect()
            self.reusable_result_buffer.clear();
            self.reusable_result_buffer.reserve(target_samples);
            self.reusable_result_buffer.extend(self.accumulator.drain(0..target_samples));
            let extracted = self.reusable_result_buffer.clone(); // Final unavoidable clone for API

            static DRAIN_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let count = DRAIN_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if count < 5 || count % 1000 == 0 {
                println!("🚰 ACCUMULATOR_DRAIN: Extracted {} samples without processing (remaining: {}, call #{})",
                         extracted.len(), self.accumulator.len(), count);
            }

            extracted
        } else {
            // Not enough samples yet - return empty
            Vec::new()
        }
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
    /// **PERFORMANCE FIX**: Reusable buffers to eliminate allocations in convert()
    reusable_input_f64: Vec<f64>,
    reusable_result_f32: Vec<f32>,
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
            reusable_input_f64: Vec::with_capacity(max_input_size),
            reusable_result_f32: Vec::with_capacity(max_output_size),
        })
    }

    /// Convert input samples with broadcast-quality transparent resampling
    ///
    /// Uses r8brain's **automatic latency removal** and **pull processing**
    /// designed specifically for real-time audio callbacks like CoreAudio.
    /// Uses dynamic output sizing - no fixed output size calculation needed.
    ///
    /// # Arguments
    /// * `input_samples` - Input audio samples at input_rate
    ///
    /// # Returns
    /// Vector with dynamic length determined by r8brain's actual output
    /// **NO PERCEIVED LATENCY** - latency is automatically compensated
    pub fn convert(&mut self, input_samples: &[f32]) -> Vec<f32> {
        // Handle empty input
        if input_samples.is_empty() {
            return Vec::new();
        }

        // Ensure we don't exceed buffer capacity
        let input_len = input_samples.len().min(self.max_input_size);

        // **PERFORMANCE FIX**: Use reusable buffer to eliminate allocation
        self.reusable_input_f64.clear();
        self.reusable_input_f64.reserve(input_len);
        for &sample in input_samples.iter().take(input_len) {
            self.reusable_input_f64.push(sample as f64);
        }

        // Process with r8brain professional resampler
        // Note: r8brain may need multiple calls before yielding output (this is normal)
        let output_len = self.resampler.process(&self.reusable_input_f64, &mut self.output_buffer);

        // **PERFORMANCE FIX**: Use reusable buffer to eliminate allocation
        if output_len > 0 {
            // Convert using reusable buffer instead of collect()
            self.reusable_result_f32.clear();
            self.reusable_result_f32.reserve(output_len);
            for &sample in self.output_buffer.iter().take(output_len) {
                self.reusable_result_f32.push(sample as f32);
            }
            self.reusable_result_f32.clone() // Final unavoidable clone for API
        } else {
            // r8brain hasn't produced output yet (normal during initial processing)
            Vec::new()
        }
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
