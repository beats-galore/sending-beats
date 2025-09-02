use super::{flush_denormal, validate_float};

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
        let output = (input_safe + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2)
            / self.a0;

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
