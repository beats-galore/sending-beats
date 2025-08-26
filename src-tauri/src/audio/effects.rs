/// Audio stability constants for denormal protection
const DENORMAL_THRESHOLD: f32 = 1e-15;
const MIN_DB: f32 = -100.0;
const MAX_DB: f32 = 40.0;
const MIN_LOG_INPUT: f32 = 1e-10;

/// **BASS POPPING FIX**: More aggressive denormal protection for filter stability
#[inline]
fn flush_denormal(x: f32) -> f32 {
    let abs_x = x.abs();
    if abs_x < DENORMAL_THRESHOLD || !x.is_finite() {
        0.0
    } else if abs_x > 100.0 {
        // Clamp extreme values that could cause instability
        if x > 0.0 { 100.0 } else { -100.0 }
    } else {
        x
    }
}

/// Safe logarithm with denormal protection
#[inline]
fn safe_log10(x: f32) -> f32 {
    if x > MIN_LOG_INPUT {
        x.log10()
    } else {
        MIN_LOG_INPUT.log10()
    }
}

/// Safe dB conversion with clamping
#[inline]
fn safe_db_to_linear(db: f32) -> f32 {
    let clamped_db = db.clamp(MIN_DB, MAX_DB);
    10.0_f32.powf(clamped_db / 20.0)
}

/// Clamp and validate floating point values
#[inline]
fn validate_float(x: f32) -> f32 {
    if x.is_finite() {
        flush_denormal(x)
    } else {
        0.0
    }
}

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
    dc_blocker: BiquadFilter,  // **BASS POPPING FIX**: DC offset removal
    equalizer: ThreeBandEqualizer,
    compressor: Compressor,
    limiter: Limiter,
    enabled: bool,
}

impl AudioEffectsChain {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            dc_blocker: BiquadFilter::high_pass(sample_rate, 20.0, 0.7), // Remove DC and sub-20Hz
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

        // **BASS POPPING FIX**: Process in order: DC Blocker -> EQ -> Compressor -> Limiter
        for sample in samples.iter_mut() {
            *sample = self.dc_blocker.process(*sample);
        }
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
    
    /// Reset all effects to prevent accumulated instabilities
    pub fn reset(&mut self) {
        self.dc_blocker.reset();  // **BASS POPPING FIX**: Reset DC blocker too
        self.equalizer.reset();
        self.compressor.reset();
        self.limiter.reset();
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
        // **BASS POPPING FIX**: Update coefficients without destroying filter state
        match band {
            EQBand::Low => {
                self.low_shelf.update_low_shelf_coeffs(self.sample_rate, 200.0, 0.7, gain_db);
            }
            EQBand::Mid => {
                self.mid_peak.update_peak_coeffs(self.sample_rate, 1000.0, 0.7, gain_db);
            }
            EQBand::High => {
                self.high_shelf.update_high_shelf_coeffs(self.sample_rate, 8000.0, 0.7, gain_db);
            }
        }
    }
    
    /// Reset all EQ filter states to prevent instabilities
    pub fn reset(&mut self) {
        self.low_shelf.reset();
        self.mid_peak.reset();
        self.high_shelf.reset();
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
    
    /// **BASS POPPING FIX**: High-pass filter for DC blocking
    pub fn high_pass(sample_rate: u32, freq: f32, q: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self {
            a0: b0 / a0,
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
        let input_safe = validate_float(input);
        let output = (input_safe + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2) / self.a0;

        // **STABILITY**: Update delay line with denormal protection
        self.x2 = flush_denormal(self.x1);
        self.x1 = input_safe;
        self.y2 = flush_denormal(self.y1);
        self.y1 = validate_float(output);

        validate_float(output)
    }
    
    /// Reset filter state to prevent instability
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
    
    /// **BASS POPPING FIX**: Update low shelf coefficients without destroying delay line
    pub fn update_low_shelf_coeffs(&mut self, sample_rate: u32, freq: f32, q: f32, gain_db: f32) {
        let gain = 10.0_f32.powf(gain_db / 20.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let beta = (gain / q).sqrt();

        let b1 = 2.0 * gain * ((gain - 1.0) - (gain + 1.0) * cos_w0);
        let b2 = gain * ((gain + 1.0) - (gain - 1.0) * cos_w0 - beta * sin_w0);
        let a0 = (gain + 1.0) + (gain - 1.0) * cos_w0 + beta * sin_w0;
        let a1 = -2.0 * ((gain - 1.0) + (gain + 1.0) * cos_w0);
        let a2 = (gain + 1.0) + (gain - 1.0) * cos_w0 - beta * sin_w0;

        // Update coefficients only - preserve delay line state!
        self.a0 = a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
    }
    
    /// **BASS POPPING FIX**: Update high shelf coefficients without destroying delay line
    pub fn update_high_shelf_coeffs(&mut self, sample_rate: u32, freq: f32, q: f32, gain_db: f32) {
        let gain = 10.0_f32.powf(gain_db / 20.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let beta = (gain / q).sqrt();

        let b1 = -2.0 * gain * ((gain - 1.0) + (gain + 1.0) * cos_w0);
        let b2 = gain * ((gain + 1.0) + (gain - 1.0) * cos_w0 - beta * sin_w0);
        let a0 = (gain + 1.0) - (gain - 1.0) * cos_w0 + beta * sin_w0;
        let a1 = 2.0 * ((gain - 1.0) - (gain + 1.0) * cos_w0);
        let a2 = (gain + 1.0) - (gain - 1.0) * cos_w0 - beta * sin_w0;

        // Update coefficients only - preserve delay line state!
        self.a0 = a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
    }
    
    /// **BASS POPPING FIX**: Update peak coefficients without destroying delay line
    pub fn update_peak_coeffs(&mut self, sample_rate: u32, freq: f32, q: f32, gain_db: f32) {
        let gain = 10.0_f32.powf(gain_db / 20.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * gain;
        let a0 = 1.0 + alpha / gain;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / gain;

        // Update coefficients only - preserve delay line state!
        self.a0 = a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
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

        compressor.set_attack(10.0); // 10ms attack - slower to prevent bass pumping
        compressor.set_release(200.0); // 200ms release - slower for smoother compression
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
            let input_safe = validate_float(*sample);
            let input_level = input_safe.abs();
            
            // **STABILITY**: Safe dB conversion with denormal protection
            let input_level_db = if input_level > MIN_LOG_INPUT {
                (20.0 * safe_log10(input_level)).clamp(MIN_DB, MAX_DB)
            } else {
                MIN_DB
            };

            // Envelope follower with stability checks
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

            self.envelope = validate_float(target_envelope + (self.envelope - target_envelope) * coeff);

            // **STABILITY**: Compression calculation with clamping
            if self.envelope > self.threshold {
                let over_threshold = self.envelope - self.threshold;
                let compressed = over_threshold / self.ratio.max(1.0); // Prevent divide by values < 1
                self.gain_reduction = (over_threshold - compressed).clamp(0.0, 60.0); // Limit max reduction
            } else {
                self.gain_reduction = 0.0;
            }

            // **STABILITY**: Apply gain reduction with safe conversion
            let gain = safe_db_to_linear(-self.gain_reduction).clamp(0.001, 2.0); // Prevent extreme gains
            *sample = validate_float(input_safe * gain);
        }
    }
    
    /// Reset compressor state to prevent instability
    pub fn reset(&mut self) {
        self.envelope = MIN_DB;
        self.gain_reduction = 0.0;
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
        let lookahead_samples = (sample_rate as f32 * 0.002) as usize; // **BASS POPPING FIX**: 2ms lookahead - reduces transient artifacts
        
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
            let input_safe = validate_float(*sample);
            
            // **STABILITY**: Store validated input in delay line
            self.delay_line[self.delay_index] = input_safe;
            
            // Get delayed sample for output
            let delayed_sample = validate_float(self.delay_line[(self.delay_index + 1) % self.delay_line.len()]);
            
            // **STABILITY**: Calculate input level in dB with protection
            let input_level = input_safe.abs();
            let input_level_db = if input_level > MIN_LOG_INPUT {
                (20.0 * safe_log10(input_level)).clamp(MIN_DB, MAX_DB)
            } else {
                MIN_DB
            };

            // Peak detection with lookahead and validation
            let target_envelope = input_level_db.max(self.envelope);
            
            // **STABILITY**: Smooth envelope with denormal protection
            self.envelope = validate_float(target_envelope + (self.envelope - target_envelope) * self.release_coeff);

            // **STABILITY**: Calculate gain reduction with clamping
            let gain_reduction = if self.envelope > self.threshold {
                (self.envelope - self.threshold).clamp(0.0, 60.0) // Limit max reduction
            } else {
                0.0
            };

            // **STABILITY**: Apply limiting with safe conversion
            let gain = safe_db_to_linear(-gain_reduction).clamp(0.001, 1.0); // Prevent amplification
            *sample = validate_float(delayed_sample * gain);

            // Advance delay line
            self.delay_index = (self.delay_index + 1) % self.delay_line.len();
        }
    }
    
    /// Reset limiter state to prevent instability
    pub fn reset(&mut self) {
        self.envelope = MIN_DB;
        self.delay_line.fill(0.0);
        self.delay_index = 0;
    }
}