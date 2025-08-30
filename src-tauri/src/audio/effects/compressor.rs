use super::{validate_float, safe_log10, safe_db_to_linear, MIN_DB, MIN_LOG_INPUT};

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
                (20.0 * safe_log10(input_level)).clamp(MIN_DB, 40.0)
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