/// Sample Rate Converter for Dynamic Audio Buffer Conversion
/// 
/// Handles real-time sample rate conversion between input and output devices
/// Supports both upsampling (interpolation) and downsampling (decimation)
/// Optimized for low-latency audio processing in callback contexts

use std::collections::VecDeque;

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

    pub fn conversion_needed(&self) -> bool {
        (self.ratio - 1.0).abs() > 0.001
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
    
    /// Get common audio sample rates
    pub fn common_sample_rates() -> &'static [u32] {
        &[8000, 11025, 16000, 22050, 44100, 48000, 88200, 96000, 176400, 192000]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_linear_src_upsampling() {
        let mut src = LinearSRC::new(44100.0, 48000.0);
        let input = vec![1.0, 0.0, -1.0, 0.0]; // 4 samples at 44.1kHz
        let output = src.convert(&input, 5); // Should produce ~4.4 samples
        
        assert_eq!(output.len(), 5);
        // First sample should be close to input
        assert!((output[0] - 1.0).abs() < 0.1);
    }
    
    #[test]  
    fn test_linear_src_downsampling() {
        let mut src = LinearSRC::new(48000.0, 44100.0);
        let input = vec![1.0, 0.5, 0.0, -0.5, -1.0]; // 5 samples at 48kHz
        let output = src.convert(&input, 4); // Should produce ~4.6 samples
        
        assert_eq!(output.len(), 4);
    }
    
    #[test]
    fn test_no_conversion_needed() {
        let src = LinearSRC::new(48000.0, 48000.0);
        assert!(!src.conversion_needed());
        
        let src2 = LinearSRC::new(44100.0, 48000.0);
        assert!(src2.conversion_needed());
    }
}