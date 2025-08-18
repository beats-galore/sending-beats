/// Real-time audio analysis
#[derive(Debug)]
pub struct AudioAnalyzer {
    peak_detector: PeakDetector,
    rms_detector: RmsDetector,
    spectrum_analyzer: Option<SpectrumAnalyzer>,
}

impl AudioAnalyzer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            peak_detector: PeakDetector::new(),
            rms_detector: RmsDetector::new(sample_rate),
            spectrum_analyzer: Some(SpectrumAnalyzer::new(sample_rate, 1024)),
        }
    }

    pub fn process(&mut self, samples: &[f32]) -> (f32, f32) {
        let peak = self.peak_detector.process(samples);
        let rms = self.rms_detector.process(samples);
        
        if let Some(ref mut analyzer) = self.spectrum_analyzer {
            analyzer.process(samples);
        }
        
        (peak, rms)
    }
    
    pub fn get_spectrum(&self) -> Option<&[f32]> {
        self.spectrum_analyzer.as_ref().map(|analyzer| analyzer.get_spectrum())
    }
}

/// Peak level detector with decay
#[derive(Debug)]
pub struct PeakDetector {
    peak: f32,
    decay_factor: f32,
}

impl PeakDetector {
    pub fn new() -> Self {
        Self {
            peak: 0.0,
            decay_factor: 0.999, // Slow decay for visual meters
        }
    }

    pub fn process(&mut self, samples: &[f32]) -> f32 {
        for &sample in samples {
            let abs_sample = sample.abs();
            if abs_sample > self.peak {
                self.peak = abs_sample;
            }
        }
        
        // Apply decay
        self.peak *= self.decay_factor;
        self.peak
    }
}

/// RMS level detector for average loudness
#[derive(Debug)]
pub struct RmsDetector {
    window_size: usize,
    sample_buffer: Vec<f32>,
    write_index: usize,
    sum_of_squares: f32,
}

impl RmsDetector {
    pub fn new(sample_rate: u32) -> Self {
        let window_size = (sample_rate as f32 * 0.1) as usize; // 100ms window
        Self {
            window_size,
            sample_buffer: vec![0.0; window_size],
            write_index: 0,
            sum_of_squares: 0.0,
        }
    }

    pub fn process(&mut self, samples: &[f32]) -> f32 {
        for &sample in samples {
            // Remove old sample from sum
            let old_sample = self.sample_buffer[self.write_index];
            self.sum_of_squares -= old_sample * old_sample;
            
            // Add new sample
            self.sample_buffer[self.write_index] = sample;
            self.sum_of_squares += sample * sample;
            
            // Advance write index
            self.write_index = (self.write_index + 1) % self.window_size;
        }
        
        (self.sum_of_squares / self.window_size as f32).sqrt()
    }
}

/// Professional spectrum analyzer for real-time frequency domain analysis
pub struct SpectrumAnalyzer {
    sample_rate: u32,
    fft_size: usize,
    window: Vec<f32>,
    input_buffer: Vec<f32>,
    output_spectrum: Vec<f32>,
    fft_planner: rustfft::FftPlanner<f32>,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    complex_buffer: Vec<rustfft::num_complex::Complex<f32>>,
}

impl SpectrumAnalyzer {
    pub fn new(sample_rate: u32, fft_size: usize) -> Self {
        // Create Hann window for better frequency resolution and reduced spectral leakage
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                let phase = 2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32;
                0.5 * (1.0 - phase.cos())
            })
            .collect();

        let mut fft_planner = rustfft::FftPlanner::new();
        let fft = fft_planner.plan_fft_forward(fft_size);

        Self {
            sample_rate,
            fft_size,
            window,
            input_buffer: vec![0.0; fft_size],
            output_spectrum: vec![0.0; fft_size / 2], // Only positive frequencies
            fft_planner,
            fft,
            complex_buffer: vec![rustfft::num_complex::Complex::new(0.0, 0.0); fft_size],
        }
    }

    pub fn process(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        // Shift the buffer left and add new samples
        let samples_to_add = samples.len().min(self.fft_size);
        if samples_to_add >= self.fft_size {
            // If we have enough samples, just use the latest ones
            self.input_buffer.copy_from_slice(&samples[samples.len() - self.fft_size..]);
        } else {
            // Shift existing samples left
            self.input_buffer.rotate_left(samples_to_add);
            // Add new samples to the end
            let start_idx = self.fft_size - samples_to_add;
            self.input_buffer[start_idx..].copy_from_slice(&samples[..samples_to_add]);
        }

        // Apply window function to reduce spectral leakage
        for (i, &sample) in self.input_buffer.iter().enumerate() {
            self.complex_buffer[i] = rustfft::num_complex::Complex::new(sample * self.window[i], 0.0);
        }

        // Perform FFT
        self.fft.process(&mut self.complex_buffer);

        // Calculate magnitude spectrum (only positive frequencies)
        for i in 0..self.output_spectrum.len() {
            let magnitude = self.complex_buffer[i].norm();
            // Convert to dB scale with floor to prevent log(0)
            self.output_spectrum[i] = if magnitude > 1e-10 {
                20.0 * magnitude.log10()
            } else {
                -100.0 // -100 dB floor
            };
        }
    }

    pub fn get_spectrum(&self) -> &[f32] {
        &self.output_spectrum
    }

    pub fn get_frequency_bins(&self) -> Vec<f32> {
        (0..self.output_spectrum.len())
            .map(|i| (i as f32 * self.sample_rate as f32) / (2.0 * self.fft_size as f32))
            .collect()
    }

    pub fn get_magnitude_at_frequency(&self, frequency: f32) -> f32 {
        let bin = ((frequency * self.fft_size as f32) / self.sample_rate as f32) as usize;
        if bin < self.output_spectrum.len() {
            self.output_spectrum[bin]
        } else {
            -100.0 // Out of range
        }
    }
}

impl std::fmt::Debug for SpectrumAnalyzer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpectrumAnalyzer")
            .field("sample_rate", &self.sample_rate)
            .field("fft_size", &self.fft_size)
            .field("window_len", &self.window.len())
            .field("input_buffer_len", &self.input_buffer.len())
            .field("output_spectrum_len", &self.output_spectrum.len())
            .finish()
    }
}

/// Real-time audio effects chain
#[derive(Debug)]
pub struct AudioEffectsChain {
    equalizer: ThreeBandEqualizer,
    compressor: Compressor,
    limiter: Limiter,
    enabled: bool,
}

impl AudioEffectsChain {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            equalizer: ThreeBandEqualizer::new(sample_rate),
            compressor: Compressor::new(sample_rate),
            limiter: Limiter::new(sample_rate),
            enabled: true,
        }
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        if !self.enabled {
            return;
        }

        // Apply effects in chain: EQ -> Compressor -> Limiter
        self.equalizer.process(samples);
        self.compressor.process(samples);
        self.limiter.process(samples);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_eq_gain(&mut self, band: EQBand, gain_db: f32) {
        self.equalizer.set_gain(band, gain_db);
    }

    pub fn set_compressor_params(&mut self, threshold: f32, ratio: f32, attack_ms: f32, release_ms: f32) {
        self.compressor.set_threshold(threshold);
        self.compressor.set_ratio(ratio);
        self.compressor.set_attack(attack_ms);
        self.compressor.set_release(release_ms);
    }

    pub fn set_limiter_threshold(&mut self, threshold_db: f32) {
        self.limiter.set_threshold(threshold_db);
    }
}

/// 3-Band Equalizer (High, Mid, Low)
#[derive(Debug, Clone, Copy)]
pub enum EQBand {
    Low,
    Mid,
    High,
}

#[derive(Debug)]
pub struct ThreeBandEqualizer {
    sample_rate: u32,
    low_shelf: BiquadFilter,
    mid_peak: BiquadFilter,
    high_shelf: BiquadFilter,
}

impl ThreeBandEqualizer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            low_shelf: BiquadFilter::low_shelf(sample_rate, 200.0, 0.7, 0.0),
            mid_peak: BiquadFilter::peak(sample_rate, 1000.0, 0.7, 0.0),
            high_shelf: BiquadFilter::high_shelf(sample_rate, 8000.0, 0.7, 0.0),
        }
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            *sample = self.low_shelf.process(*sample);
            *sample = self.mid_peak.process(*sample);
            *sample = self.high_shelf.process(*sample);
        }
    }

    pub fn set_gain(&mut self, band: EQBand, gain_db: f32) {
        match band {
            EQBand::Low => {
                self.low_shelf = BiquadFilter::low_shelf(self.sample_rate, 200.0, 0.7, gain_db);
            }
            EQBand::Mid => {
                self.mid_peak = BiquadFilter::peak(self.sample_rate, 1000.0, 0.7, gain_db);
            }
            EQBand::High => {
                self.high_shelf = BiquadFilter::high_shelf(self.sample_rate, 8000.0, 0.7, gain_db);
            }
        }
    }
}

/// Biquad IIR filter for EQ
#[derive(Debug)]
pub struct BiquadFilter {
    a0: f32,
    a1: f32,
    a2: f32,
    b1: f32,
    b2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadFilter {
    pub fn low_shelf(sample_rate: u32, freq: f32, q: f32, gain_db: f32) -> Self {
        let gain = 10.0_f32.powf(gain_db / 20.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let _alpha = sin_w0 / (2.0 * q);
        let _s = 1.0;
        let beta = (gain / q).sqrt();

        let _b0 = gain * ((gain + 1.0) - (gain - 1.0) * cos_w0 + beta * sin_w0);
        let b1 = 2.0 * gain * ((gain - 1.0) - (gain + 1.0) * cos_w0);
        let b2 = gain * ((gain + 1.0) - (gain - 1.0) * cos_w0 - beta * sin_w0);
        let a0 = (gain + 1.0) + (gain - 1.0) * cos_w0 + beta * sin_w0;
        let a1 = -2.0 * ((gain - 1.0) + (gain + 1.0) * cos_w0);
        let a2 = (gain + 1.0) + (gain - 1.0) * cos_w0 - beta * sin_w0;

        Self {
            a0: a0,
            a1: a1 / a0,
            a2: a2 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn high_shelf(sample_rate: u32, freq: f32, q: f32, gain_db: f32) -> Self {
        let gain = 10.0_f32.powf(gain_db / 20.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let beta = (gain / q).sqrt();

        let _b0 = gain * ((gain + 1.0) + (gain - 1.0) * cos_w0 + beta * sin_w0);
        let b1 = -2.0 * gain * ((gain - 1.0) + (gain + 1.0) * cos_w0);
        let b2 = gain * ((gain + 1.0) + (gain - 1.0) * cos_w0 - beta * sin_w0);
        let a0 = (gain + 1.0) - (gain - 1.0) * cos_w0 + beta * sin_w0;
        let a1 = 2.0 * ((gain - 1.0) - (gain + 1.0) * cos_w0);
        let a2 = (gain + 1.0) - (gain - 1.0) * cos_w0 - beta * sin_w0;

        Self {
            a0: a0,
            a1: a1 / a0,
            a2: a2 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn peak(sample_rate: u32, freq: f32, q: f32, gain_db: f32) -> Self {
        let gain = 10.0_f32.powf(gain_db / 20.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let _b0 = 1.0 + alpha * gain;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * gain;
        let a0 = 1.0 + alpha / gain;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / gain;

        Self {
            a0: a0,
            a1: a1 / a0,
            a2: a2 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let output = (input + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2) / self.a0;

        // Update delay line
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }
}

/// Dynamic range compressor
#[derive(Debug)]
pub struct Compressor {
    sample_rate: u32,
    threshold: f32,
    ratio: f32,
    attack_coeff: f32,
    release_coeff: f32,
    envelope: f32,
    gain_reduction: f32,
}

impl Compressor {
    pub fn new(sample_rate: u32) -> Self {
        let mut compressor = Self {
            sample_rate,
            threshold: -12.0, // dB
            ratio: 4.0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            envelope: 0.0,
            gain_reduction: 0.0,
        };

        compressor.set_attack(5.0); // 5ms attack
        compressor.set_release(100.0); // 100ms release
        compressor
    }

    pub fn set_threshold(&mut self, threshold_db: f32) {
        self.threshold = threshold_db;
    }

    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.max(1.0);
    }

    pub fn set_attack(&mut self, attack_ms: f32) {
        self.attack_coeff = (-1.0 / (attack_ms * 0.001 * self.sample_rate as f32)).exp();
    }

    pub fn set_release(&mut self, release_ms: f32) {
        self.release_coeff = (-1.0 / (release_ms * 0.001 * self.sample_rate as f32)).exp();
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            let input_level = sample.abs();
            let input_level_db = if input_level > 0.0 {
                20.0 * input_level.log10()
            } else {
                -100.0
            };

            // Envelope follower
            let target_envelope = if input_level_db > self.envelope {
                input_level_db
            } else {
                self.envelope
            };

            let coeff = if input_level_db > self.envelope {
                self.attack_coeff
            } else {
                self.release_coeff
            };

            self.envelope = target_envelope + (self.envelope - target_envelope) * coeff;

            // Compression calculation
            if self.envelope > self.threshold {
                let over_threshold = self.envelope - self.threshold;
                let compressed = over_threshold / self.ratio;
                self.gain_reduction = over_threshold - compressed;
            } else {
                self.gain_reduction = 0.0;
            }

            // Apply gain reduction
            let gain = 10.0_f32.powf(-self.gain_reduction / 20.0);
            *sample *= gain;
        }
    }
}

/// Brick-wall limiter
#[derive(Debug)]
pub struct Limiter {
    sample_rate: u32,
    threshold: f32,
    release_coeff: f32,
    envelope: f32,
    delay_line: Vec<f32>,
    delay_index: usize,
}

impl Limiter {
    pub fn new(sample_rate: u32) -> Self {
        let lookahead_samples = (sample_rate as f32 * 0.005) as usize; // 5ms lookahead
        
        let mut limiter = Self {
            sample_rate,
            threshold: -0.1, // dB
            release_coeff: 0.0,
            envelope: 0.0,
            delay_line: vec![0.0; lookahead_samples],
            delay_index: 0,
        };

        limiter.set_release(50.0); // 50ms release
        limiter
    }

    pub fn set_threshold(&mut self, threshold_db: f32) {
        self.threshold = threshold_db;
    }

    pub fn set_release(&mut self, release_ms: f32) {
        self.release_coeff = (-1.0 / (release_ms * 0.001 * self.sample_rate as f32)).exp();
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            // Store input in delay line
            self.delay_line[self.delay_index] = *sample;
            
            // Get delayed sample for output
            let delayed_sample = self.delay_line[(self.delay_index + 1) % self.delay_line.len()];
            
            // Calculate input level in dB
            let input_level_db = if sample.abs() > 0.0 {
                20.0 * sample.abs().log10()
            } else {
                -100.0
            };

            // Peak detection with lookahead
            let target_envelope = input_level_db.max(self.envelope);
            
            // Smooth envelope
            self.envelope = target_envelope + (self.envelope - target_envelope) * self.release_coeff;

            // Calculate gain reduction
            let gain_reduction = if self.envelope > self.threshold {
                self.envelope - self.threshold
            } else {
                0.0
            };

            // Apply limiting
            let gain = 10.0_f32.powf(-gain_reduction / 20.0);
            *sample = delayed_sample * gain;

            // Advance delay line
            self.delay_index = (self.delay_index + 1) % self.delay_line.len();
        }
    }
}