use super::{validate_float, safe_log10, safe_db_to_linear, MIN_DB, MIN_LOG_INPUT};

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
                (20.0 * safe_log10(input_level)).clamp(MIN_DB, 40.0)
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