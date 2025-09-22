use anyhow::Result;
use colored::*;
use r8brain_rs::{PrecisionProfile, Resampler};
use std::collections::VecDeque;
use tracing::{info, warn};

/// r8brain-based Sample Rate Converter using proper r8brain-rs API
///
/// This resampler properly uses the r8brain.process() method for high-quality
/// professional audio resampling with sinc filters instead of terrible linear interpolation.
pub struct R8brainSRC {
    /// Input sample rate
    input_rate: f64,
    /// Output sample rate
    output_rate: f64,
    /// Conversion ratio (input_rate / output_rate)
    ratio: f64,
    /// r8brain resampler instance (actually used this time!)
    resampler: Resampler,
    /// Number of channels (always 2 for stereo)
    channels: usize,
    /// Input buffer for f32 -> f64 conversion
    input_buffer: Vec<f64>,
    /// Output buffer for r8brain processing (f64)
    output_buffer: Vec<f64>,
    /// Accumulated output samples waiting to be consumed (f32)
    accumulated_output: VecDeque<f32>,
}

impl std::fmt::Debug for R8brainSRC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("R8brainSRC")
            .field("input_rate", &self.input_rate)
            .field("output_rate", &self.output_rate)
            .field("ratio", &self.ratio)
            .field("channels", &self.channels)
            .field("output_buffer_capacity", &self.output_buffer.capacity())
            .field("accumulated_samples", &self.accumulated_output.len())
            .field("resampler", &"r8brain_rs::Resampler")
            .finish()
    }
}

impl R8brainSRC {
    /// Create a new r8brain-based resampler
    ///
    /// Uses the actual r8brain library for professional-quality resampling
    /// instead of terrible linear interpolation.
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz (e.g., 48000)
    /// * `output_rate` - Output sample rate in Hz (e.g., 44100)
    /// * `chunk_size` - Desired output chunk size in frames (used for buffer calculation)
    ///
    /// # Returns
    /// r8brain-based resampler ready for streaming operation
    pub fn new(input_rate: u32, output_rate: u32) -> Result<Self, String> {
        let input_rate_f64 = input_rate as f64;
        let output_rate_f64 = output_rate as f64;
        let ratio = input_rate_f64 / output_rate_f64;

        info!(
            "🎯 {}: Creating REAL r8brain resampler {}Hz→{}Hz (ratio: {:.4})",
            "R8BRAIN_INIT".on_blue().yellow(),
            input_rate,
            output_rate,
            ratio
        );

        // Create r8brain resampler with high quality settings
        let resampler = Resampler::new(
            input_rate_f64,
            output_rate_f64,
            4096,                     // Max block size
            2.0,                      // Transition band (higher = faster, lower = better quality)
            PrecisionProfile::Bits24, // High quality
        );

        // Calculate worst-case output buffer size
        // For downsampling, we might get less. For upsampling, we might get more.
        let max_output_size = if ratio < 1.0 {
            // Upsampling: could get up to 3x input size in worst case
            (4096.0 / ratio * 3.0) as usize
        } else {
            // Downsampling: typically less than input, but allocate some safety
            6144
        };

        info!(
            "🎯 {}: Using actual r8brain library (max_output_buffer: {} samples)",
            "R8BRAIN_BUFFER".on_blue().yellow(),
            max_output_size
        );

        Ok(Self {
            input_rate: input_rate_f64,
            output_rate: output_rate_f64,
            ratio,
            resampler,
            channels: 2,
            input_buffer: Vec::with_capacity(4096),
            output_buffer: vec![0.0; max_output_size],
            accumulated_output: VecDeque::new(),
        })
    }

    /// Convert input samples to output sample rate using r8brain
    /// Stateless operation that processes input and returns output immediately
    ///
    /// # Arguments
    /// * `input_samples` - Input audio samples at input_rate (interleaved stereo)
    ///
    /// # Returns
    /// Vector of resampled audio with length determined by r8brain conversion
    pub fn convert(&mut self, input_samples: &[f32]) -> Vec<f32> {
        if input_samples.is_empty() {
            return Vec::new();
        }

        // Convert f32 input to f64 for r8brain
        self.input_buffer.clear();
        self.input_buffer
            .extend(input_samples.iter().map(|&x| x as f64));

        let output_len = self
            .resampler
            .process(&self.input_buffer, &mut self.output_buffer);

        // r8brain produced some output - convert f64 back to f32 and return immediately
        if output_len > 0 {
            let output_samples: Vec<f32> = self.output_buffer[..output_len]
                .iter()
                .map(|&x| x as f32)
                .collect();

            static PROCESS_LOG_COUNT: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let process_count =
                PROCESS_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            if process_count < 5 || process_count % 1000 == 0 {
                info!(
                    "🎯 {}: r8brain converted {} → {} samples (stateless)",
                    "R8BRAIN_CONVERT".on_blue().yellow(),
                    input_samples.len(),
                    output_len
                );
            }

            output_samples
        } else {
            Vec::new()
        }
    }

    /// Read output samples from r8brain-processed accumulation buffer
    ///
    /// Drains available resampled samples from the internal buffer.
    /// Returns whatever is available, no complex buffering logic.
    ///
    /// # Arguments
    /// * `output_count` - Number of output samples requested (stereo interleaved)
    ///
    /// # Returns
    /// Vector of available resampled samples (may be less than requested)
    pub fn read_output_samples(&mut self, output_count: usize) -> Vec<f32> {
        let available = self.accumulated_output.len();
        let to_take = std::cmp::min(output_count, available);

        // Drain the available samples from accumulated output
        let mut output = Vec::with_capacity(to_take);
        for _ in 0..to_take {
            if let Some(sample) = self.accumulated_output.pop_front() {
                output.push(sample);
            }
        }

        // Log when we don't have enough samples (this is normal for rate mismatch)
        if to_take < output_count {
            static STARVE_LOG_COUNT: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let starve_count = STARVE_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            if starve_count < 10 || starve_count % 100 == 0 {
                info!(
                    "🎯 {}: Provided {} / {} samples requested (buffer had {})",
                    "R8BRAIN_PARTIAL".on_blue().yellow(),
                    to_take,
                    output_count,
                    available
                );
            }
        }

        output
    }

    /// Check if resampler is ready to produce output
    ///
    /// Returns true when we have accumulated samples to output
    pub fn is_ready(&self) -> bool {
        !self.accumulated_output.is_empty()
    }

    /// Get current buffer fill level (for monitoring)
    pub fn buffer_fill_ratio(&self) -> f32 {
        // Report fill based on accumulated output samples
        self.accumulated_output.len() as f32 / 1000.0 // Normalize to reasonable range
    }

    /// Adjust conversion ratio for clock drift compensation
    ///
    /// Note: r8brain doesn't support runtime ratio adjustment,
    /// so this just updates our internal tracking.
    ///
    /// # Arguments
    /// * `ratio_adjustment` - Small adjustment to ratio (e.g., 0.0001)
    pub fn adjust_ratio(&mut self, ratio_adjustment: f64) {
        let old_ratio = self.ratio;
        self.ratio += ratio_adjustment;

        info!(
            "🎯 {}: Ratio adjusted {:.6} → {:.6} (delta: {:+.6}) [r8brain doesn't support runtime adjustment]",
            "R8BRAIN_DRIFT_COMP".on_blue().yellow(),
            old_ratio,
            self.ratio,
            ratio_adjustment
        );
    }

    /// Get conversion ratio (for debugging and compatibility)
    pub fn ratio(&self) -> f32 {
        self.ratio as f32
    }

    /// Check if conversion is needed (rates are different)
    pub fn conversion_needed(&self) -> bool {
        (self.ratio - 1.0).abs() > 0.001
    }

    /// Get input sample rate
    pub fn input_rate(&self) -> u32 {
        self.input_rate as u32
    }

    /// Get output sample rate
    pub fn output_rate(&self) -> u32 {
        self.output_rate as u32
    }

    /// Reset the resampler state
    pub fn reset(&mut self) {
        self.accumulated_output.clear();
        // Note: r8brain doesn't expose a reset method, so we keep internal state
        info!(
            "🎯 {}: Resampler state reset (cleared accumulated output)",
            "R8BRAIN_RESET".on_blue().yellow()
        );
    }

    /// Get estimated latency in samples at output rate
    pub fn output_delay(&self) -> f32 {
        // r8brain manages internal buffering, we just report accumulated samples + typical delay
        self.accumulated_output.len() as f32 + 64.0 // r8brain typical internal delay
    }

    /// Calculate number of input frames needed to produce desired output frames
    /// This provides consistent API with rubato's input_frames_next functionality
    ///
    /// # Arguments
    /// * `desired_output_frames` - Number of output frames desired
    ///
    /// # Returns
    /// Number of input frames needed (estimated based on conversion ratio)
    pub fn input_frames_needed(&self, desired_output_frames: usize) -> usize {
        // For r8brain, estimate input frames needed based on conversion ratio
        // Add small buffer for safety since r8brain can have variable behavior
        let estimated_input = (desired_output_frames as f64 * self.ratio * 1.1) as usize;

        // Ensure minimum reasonable buffer size
        estimated_input.max(64)
    }

    /// Update sample rates (useful for dynamic rate changes)
    pub fn update_rates(&mut self, input_rate: u32, output_rate: u32) -> Result<(), String> {
        let new_input_rate = input_rate as f64;
        let new_output_rate = output_rate as f64;
        let new_ratio = new_input_rate / new_output_rate;

        info!(
            "🎯 {}: Updated rates to {}Hz→{}Hz (ratio: {:.3})",
            "R8BRAIN_UPDATE".cyan(),
            input_rate,
            output_rate,
            new_ratio as f32
        );

        // Create new resampler with updated rates
        let new_resampler = Resampler::new(
            new_input_rate,
            new_output_rate,
            4096,                     // Max block size
            2.0,                      // Transition band
            PrecisionProfile::Bits24, // High quality
        );

        self.input_rate = new_input_rate;
        self.output_rate = new_output_rate;
        self.ratio = new_ratio;
        self.resampler = new_resampler;

        // Reset state for clean transition
        self.reset();

        Ok(())
    }
}
