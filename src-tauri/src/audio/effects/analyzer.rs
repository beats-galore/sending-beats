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