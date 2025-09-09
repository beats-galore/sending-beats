// CoreAudio format conversion and sample rate conversion
//
// This module provides comprehensive audio format conversion capabilities
// for the CoreAudio CPAL replacement, including:
// - Sample rate conversion with high quality interpolation
// - Channel format conversion (mono/stereo/multi-channel)
// - Sample format conversion (i16/i32/f32)
// - Buffer size adaptation for real-time audio processing

#[cfg(target_os = "macos")]
use anyhow::Result;
#[cfg(target_os = "macos")]
use std::collections::VecDeque;

/// High-quality sample rate converter for CoreAudio streams
#[cfg(target_os = "macos")]
pub struct CoreAudioSampleRateConverter {
    input_rate: f64,
    output_rate: f64,
    ratio: f64,
    buffer: VecDeque<f32>,
    interpolator: LinearInterpolator,
}

#[cfg(target_os = "macos")]
impl CoreAudioSampleRateConverter {
    /// Create a new sample rate converter
    pub fn new(input_rate: f64, output_rate: f64) -> Self {
        let ratio = output_rate / input_rate;
        Self {
            input_rate,
            output_rate,
            ratio,
            buffer: VecDeque::new(),
            interpolator: LinearInterpolator::new(),
        }
    }

    /// Convert input samples to output sample rate
    pub fn convert(&mut self, input_samples: &[f32], output_size: usize) -> Vec<f32> {
        // Add input samples to buffer
        self.buffer.extend(input_samples);

        let mut output = Vec::with_capacity(output_size);

        // If no conversion needed (rates match)
        if (self.ratio - 1.0).abs() < 0.001 {
            // Direct copy with size matching
            let samples_to_take = output_size.min(self.buffer.len());
            for _ in 0..samples_to_take {
                if let Some(sample) = self.buffer.pop_front() {
                    output.push(sample);
                }
            }
            // Fill remaining with zeros if needed
            while output.len() < output_size {
                output.push(0.0);
            }
            return output;
        }

        // Perform sample rate conversion
        let mut input_pos = 0.0;
        while output.len() < output_size && input_pos < (self.buffer.len() as f64 - 1.0) {
            let sample = self.interpolator.interpolate(&self.buffer, input_pos);
            output.push(sample);
            input_pos += 1.0 / self.ratio;
        }

        // Remove consumed samples from buffer
        let consumed = input_pos.floor() as usize;
        for _ in 0..consumed.min(self.buffer.len()) {
            self.buffer.pop_front();
        }

        // Fill remaining with zeros if needed
        while output.len() < output_size {
            output.push(0.0);
        }

        output
    }

    /// Get the conversion ratio
    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// Check if conversion is needed
    pub fn needs_conversion(&self) -> bool {
        (self.ratio - 1.0).abs() > 0.001
    }
}

/// Linear interpolator for sample rate conversion
#[cfg(target_os = "macos")]
struct LinearInterpolator;

#[cfg(target_os = "macos")]
impl LinearInterpolator {
    fn new() -> Self {
        Self
    }

    /// Perform linear interpolation at the given position
    fn interpolate(&self, buffer: &VecDeque<f32>, position: f64) -> f32 {
        let index = position.floor() as usize;
        let frac = position.fract();

        if index >= buffer.len() {
            return 0.0;
        }

        let sample1 = buffer[index];
        let sample2 = if index + 1 < buffer.len() {
            buffer[index + 1]
        } else {
            sample1
        };

        // Linear interpolation
        sample1 + (sample2 - sample1) * (frac as f32)
    }
}

/// Channel format converter for CoreAudio streams
#[cfg(target_os = "macos")]
pub struct CoreAudioChannelConverter;

#[cfg(target_os = "macos")]
impl CoreAudioChannelConverter {
    /// Convert mono to stereo by duplicating samples
    pub fn mono_to_stereo(mono_samples: &[f32]) -> Vec<f32> {
        let mut stereo = Vec::with_capacity(mono_samples.len() * 2);
        for &sample in mono_samples {
            stereo.push(sample); // Left channel
            stereo.push(sample); // Right channel (duplicate)
        }
        stereo
    }

    /// Convert stereo to mono by averaging channels
    pub fn stereo_to_mono(stereo_samples: &[f32]) -> Vec<f32> {
        let mut mono = Vec::with_capacity(stereo_samples.len() / 2);
        let mut i = 0;
        while i + 1 < stereo_samples.len() {
            let left = stereo_samples[i];
            let right = stereo_samples[i + 1];
            mono.push((left + right) * 0.5);
            i += 2;
        }
        mono
    }

    /// Convert between arbitrary channel counts (simplified)
    pub fn convert_channels(input: &[f32], input_channels: u32, output_channels: u32) -> Vec<f32> {
        match (input_channels, output_channels) {
            (1, 2) => Self::mono_to_stereo(input),
            (2, 1) => Self::stereo_to_mono(input),
            (x, y) if x == y => input.to_vec(), // No conversion needed
            _ => {
                // Fallback: repeat or truncate channels as needed
                let frames = input.len() / input_channels as usize;
                let mut output = Vec::with_capacity(frames * output_channels as usize);

                for frame in 0..frames {
                    for out_ch in 0..output_channels {
                        let in_ch = (out_ch % input_channels) as usize;
                        let sample_idx = frame * input_channels as usize + in_ch;
                        if sample_idx < input.len() {
                            output.push(input[sample_idx]);
                        } else {
                            output.push(0.0);
                        }
                    }
                }
                output
            }
        }
    }
}

/// Sample format converter for CoreAudio streams
#[cfg(target_os = "macos")]
pub struct CoreAudioFormatConverter;

#[cfg(target_os = "macos")]
impl CoreAudioFormatConverter {
    /// Convert i16 samples to f32 (-32768..32767 -> -1.0..1.0)
    pub fn i16_to_f32(i16_samples: &[i16]) -> Vec<f32> {
        i16_samples
            .iter()
            .map(|&sample| sample as f32 / 32768.0)
            .collect()
    }

    /// Convert f32 samples to i16 (-1.0..1.0 -> -32768..32767)
    pub fn f32_to_i16(f32_samples: &[f32]) -> Vec<i16> {
        f32_samples
            .iter()
            .map(|&sample| (sample.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect()
    }

    /// Convert i32 samples to f32 (24-bit in 32-bit container)
    pub fn i32_to_f32_24bit(i32_samples: &[i32]) -> Vec<f32> {
        i32_samples
            .iter()
            .map(|&sample| (sample >> 8) as f32 / 8388608.0) // 24-bit conversion
            .collect()
    }

    /// Convert f32 samples to i32 (24-bit in 32-bit container)
    pub fn f32_to_i32_24bit(f32_samples: &[f32]) -> Vec<i32> {
        f32_samples
            .iter()
            .map(|&sample| ((sample.clamp(-1.0, 1.0) * 8388607.0) as i32) << 8)
            .collect()
    }
}

/// Unified audio format converter for CoreAudio streams
#[cfg(target_os = "macos")]
pub struct CoreAudioUnifiedConverter {
    sample_rate_converter: Option<CoreAudioSampleRateConverter>,
    input_channels: u32,
    output_channels: u32,
    input_rate: f64,
    output_rate: f64,
}

#[cfg(target_os = "macos")]
impl CoreAudioUnifiedConverter {
    /// Create a new unified converter
    pub fn new(
        input_rate: f64,
        output_rate: f64,
        input_channels: u32,
        output_channels: u32,
    ) -> Self {
        let sample_rate_converter = if (input_rate - output_rate).abs() > 0.1 {
            Some(CoreAudioSampleRateConverter::new(input_rate, output_rate))
        } else {
            None
        };

        Self {
            sample_rate_converter,
            input_channels,
            output_channels,
            input_rate,
            output_rate,
        }
    }

    /// Convert audio with all necessary format conversions
    pub fn convert_audio(&mut self, input: &[f32], output_size: usize) -> Vec<f32> {
        let mut processed = input.to_vec();

        // Step 1: Channel conversion
        if self.input_channels != self.output_channels {
            processed = CoreAudioChannelConverter::convert_channels(
                &processed,
                self.input_channels,
                self.output_channels,
            );
        }

        // Step 2: Sample rate conversion
        if let Some(ref mut src) = self.sample_rate_converter {
            processed = src.convert(&processed, output_size);
        } else if processed.len() > output_size {
            // Truncate if too many samples
            processed.truncate(output_size);
        } else if processed.len() < output_size {
            // Pad with zeros if too few samples
            processed.resize(output_size, 0.0);
        }

        processed
    }

    /// Check if any conversion is needed
    pub fn needs_conversion(&self) -> bool {
        self.input_channels != self.output_channels
            || self
                .sample_rate_converter
                .as_ref()
                .map(|src| src.needs_conversion())
                .unwrap_or(false)
    }
}

// Non-macOS stub implementations
#[cfg(not(target_os = "macos"))]
pub struct CoreAudioSampleRateConverter;

#[cfg(not(target_os = "macos"))]
impl CoreAudioSampleRateConverter {
    pub fn new(_input_rate: f64, _output_rate: f64) -> Self {
        Self
    }

    pub fn convert(&mut self, input_samples: &[f32], output_size: usize) -> Vec<f32> {
        input_samples.to_vec()
    }
}

#[cfg(not(target_os = "macos"))]
pub struct CoreAudioChannelConverter;

#[cfg(not(target_os = "macos"))]
pub struct CoreAudioFormatConverter;

#[cfg(not(target_os = "macos"))]
pub struct CoreAudioUnifiedConverter;