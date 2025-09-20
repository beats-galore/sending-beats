use rubato::{SincFixedOut, SincInterpolationParameters, SincInterpolationType, WindowFunction, Resampler};
use std::collections::VecDeque;
use anyhow::Result;

/// Stereo streaming resampler: SincFixedOut (fixed output frames)
pub struct RubatoSRC {
    resampler: SincFixedOut<f32>,
    // per-channel FIFO (deinterleaved): [0]=Left, [1]=Right
    input_fifos: [VecDeque<f32>; 2],
    channels: usize,
    input_rate: f32,
    output_rate: f32,
}

impl RubatoSRC {
    /// Create a streaming resampler that will produce `output_frames_per_channel`
    /// when asked (that's the "fixed output" size). `output_frames_per_channel`
    /// should match the device chunk size divided by `channels`.
    pub fn new_sinc_fixed_out(
        input_rate: f32,
        output_rate: f32,
        output_frames_per_channel: usize,
        channels: usize, // 2 for stereo
    ) -> Result<Self, String> {


        let params = SincInterpolationParameters {
            sinc_len: 64, // Reduced from 64 for real-time performance
            f_cutoff: 0.95, // Slightly lower for stability
            interpolation: SincInterpolationType::Cubic, // Faster than Cubic
            oversampling_factor: 32, // Reduced from 128 for speed
            window: WindowFunction::BlackmanHarris2, // Faster than BlackmanHarris2
        };

        // ratio = output / input (rubato expects that order for SincFixedOut::new)
        let ratio = output_rate as f64 / input_rate as f64;

        let resampler = SincFixedOut::<f32>::new(
            ratio,
            2.0, // Higher tolerance for real-time performance (was 2.0)
            params,
            output_frames_per_channel, // requested fixed output frames per channel
            channels,
        )
        .map_err(|e| format!("Failed creating SincFixedOut: {}", e))?;

        Ok(Self {
            resampler,
            input_fifos: [VecDeque::new(), VecDeque::new()],
            channels,
            input_rate,
            output_rate,
        })
    }

    /// Push interleaved samples (LRLR...) into the internal FIFOs.
    /// This should be called from your producer when new mixed audio arrives.
    pub fn push_interleaved(&mut self, interleaved: &[f32]) {
        if self.channels == 1 {
            // mono
            for &s in interleaved {
                self.input_fifos[0].push_back(s);
            }
            return;
        }

        let mut i = 0;
        while i + (self.channels - 1) < interleaved.len() {
            // assume interleaved layout matches self.channels
            for ch in 0..self.channels {
                self.input_fifos[ch].push_back(interleaved[i + ch]);
            }
            i += self.channels;
        }
    }

    /// Produce exactly `output_frames_per_channel * channels` interleaved samples.
    /// This is the function you call in your output callback (or output worker)
    /// where `output_frames_per_channel` equals device_chunk_size / channels.
    pub fn get_output_interleaved(&mut self, output_frames_per_channel: usize) -> Vec<f32> {
        // Ensure resampler's configured fixed output size matches requested.
        // If not, you'd need to recreate the resampler (avoid in real-time).
        // rubato::SincFixedOut::new used output_frames_per_channel at init time;
        // here we assume caller set it to that value.

        // For SincFixedOut, ask the resampler how many input frames it needs
        let needed_input_frames = self.resampler.input_frames_next();
        // If FIFO doesn't have enough, pad with zeros to avoid underrun.
        for ch in 0..self.channels {
            let missing = needed_input_frames.saturating_sub(self.input_fifos[ch].len());
            if missing > 0 {
                // push zeros
                self.input_fifos[ch].extend(std::iter::repeat(0.0f32).take(missing));
            }
        }

        // Build input Vec<Vec<f32>> per rubato API: channels x frames
        let mut input: Vec<Vec<f32>> = Vec::with_capacity(self.channels);
        for ch in 0..self.channels {
            let mut vec_ch = Vec::with_capacity(needed_input_frames);
            for _ in 0..needed_input_frames {
                // safe because we padded above
                vec_ch.push(self.input_fifos[ch].pop_front().unwrap_or(0.0));
            }
            input.push(vec_ch);
        }

        // Ask the resampler for exactly output_frames_per_channel per channel
        // note: process returns Vec<Vec<f32>> where inner vec is frames for channel
        // For SincFixedOut, the second parameter might be different
        let processed = match self.resampler.process(&input, None) {
            Ok(v) => v,
            Err(e) => {
                // on error return silence to avoid crashing audio thread
                eprintln!("Resampler process error: {}", e);
                vec![vec![0.0f32; output_frames_per_channel]; self.channels]
            }
        };

        // Interleave and return LRLR...
        let mut interleaved = Vec::with_capacity(output_frames_per_channel * self.channels);
        for frame in 0..output_frames_per_channel {
            for ch in 0..self.channels {
                interleaved.push(processed[ch][frame]);
            }
        }
        interleaved
    }

    /// Optional: query the current configured delay (samples)
    pub fn output_delay(&self) -> usize {
        self.resampler.output_delay()
    }

    /// Get input sample rate for compatibility checks
    pub fn input_rate(&self) -> f32 {
        self.input_rate
    }

    /// Get output sample rate for compatibility checks
    pub fn output_rate(&self) -> f32 {
        self.output_rate
    }
}

impl std::fmt::Debug for RubatoSRC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RubatoSRC")
            .field("channels", &self.channels)
            .field("input_rate", &self.input_rate)
            .field("output_rate", &self.output_rate)
            .field("input_fifos_len", &[self.input_fifos[0].len(), self.input_fifos[1].len()])
            .finish()
    }
}
