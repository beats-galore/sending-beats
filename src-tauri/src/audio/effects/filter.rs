// Biquad IIR filter, RBJ cookbook coefficients.
//
// Coefficients are normalized by a0 at derivation time, so the per-sample path
// is five multiplies and four adds with no division. The previous
// implementation half-normalized in the constructors and divided by a leftover
// a0 again in process(), which attenuated every stage — a shelf at 0 dB cut
// the signal roughly in half, and a chain of them buried it (issue #41).

use super::{flush_denormal, validate_float};

/// One set of normalized biquad coefficients.
#[derive(Debug, Clone, Copy)]
struct Coefficients {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Coefficients {
    fn normalized(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// RBJ high-pass.
    fn high_pass(sample_rate: u32, freq: f32, q: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q);

        Self::normalized(
            (1.0 + cos_w0) / 2.0,
            -(1.0 + cos_w0),
            (1.0 + cos_w0) / 2.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        )
    }

    /// RBJ peaking EQ. `gain_db` boosts or cuts around `freq`.
    fn peak(sample_rate: u32, freq: f32, q: f32, gain_db: f32) -> Self {
        // Amplitude convention: the cookbook's A is 10^(dB/40) for peak and
        // shelf filters, so that boost and cut of the same dB mirror exactly.
        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q);

        Self::normalized(
            1.0 + alpha * a,
            -2.0 * cos_w0,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos_w0,
            1.0 - alpha / a,
        )
    }

    /// RBJ low shelf.
    fn low_shelf(sample_rate: u32, freq: f32, q: f32, gain_db: f32) -> Self {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q);
        let beta = 2.0 * a.sqrt() * alpha;

        Self::normalized(
            a * ((a + 1.0) - (a - 1.0) * cos_w0 + beta),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0),
            a * ((a + 1.0) - (a - 1.0) * cos_w0 - beta),
            (a + 1.0) + (a - 1.0) * cos_w0 + beta,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0),
            (a + 1.0) + (a - 1.0) * cos_w0 - beta,
        )
    }

    /// RBJ high shelf.
    fn high_shelf(sample_rate: u32, freq: f32, q: f32, gain_db: f32) -> Self {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q);
        let beta = 2.0 * a.sqrt() * alpha;

        Self::normalized(
            a * ((a + 1.0) + (a - 1.0) * cos_w0 + beta),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0),
            a * ((a + 1.0) + (a - 1.0) * cos_w0 - beta),
            (a + 1.0) - (a - 1.0) * cos_w0 + beta,
            2.0 * ((a - 1.0) - (a + 1.0) * cos_w0),
            (a + 1.0) - (a - 1.0) * cos_w0 - beta,
        )
    }
}

/// Biquad IIR filter for EQ
#[derive(Debug)]
pub struct BiquadFilter {
    coefficients: Coefficients,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadFilter {
    fn with_coefficients(coefficients: Coefficients) -> Self {
        Self {
            coefficients,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    pub fn low_shelf(sample_rate: u32, freq: f32, q: f32, gain_db: f32) -> Self {
        Self::with_coefficients(Coefficients::low_shelf(sample_rate, freq, q, gain_db))
    }

    pub fn high_shelf(sample_rate: u32, freq: f32, q: f32, gain_db: f32) -> Self {
        Self::with_coefficients(Coefficients::high_shelf(sample_rate, freq, q, gain_db))
    }

    pub fn peak(sample_rate: u32, freq: f32, q: f32, gain_db: f32) -> Self {
        Self::with_coefficients(Coefficients::peak(sample_rate, freq, q, gain_db))
    }

    pub fn high_pass(sample_rate: u32, freq: f32, q: f32) -> Self {
        Self::with_coefficients(Coefficients::high_pass(sample_rate, freq, q))
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let x = validate_float(input);
        let c = self.coefficients;
        let output = c.b0 * x + c.b1 * self.x1 + c.b2 * self.x2 - c.a1 * self.y1 - c.a2 * self.y2;
        let output = validate_float(output);

        // **STABILITY**: Update delay line with denormal protection
        self.x2 = flush_denormal(self.x1);
        self.x1 = x;
        self.y2 = flush_denormal(self.y1);
        self.y1 = output;

        output
    }

    /// Reset filter state to prevent instability
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    /// Update low shelf coefficients without destroying the delay line
    pub fn update_low_shelf_coeffs(&mut self, sample_rate: u32, freq: f32, q: f32, gain_db: f32) {
        self.coefficients = Coefficients::low_shelf(sample_rate, freq, q, gain_db);
    }

    /// Update high shelf coefficients without destroying the delay line
    pub fn update_high_shelf_coeffs(&mut self, sample_rate: u32, freq: f32, q: f32, gain_db: f32) {
        self.coefficients = Coefficients::high_shelf(sample_rate, freq, q, gain_db);
    }

    /// Update peak coefficients without destroying the delay line
    pub fn update_peak_coeffs(&mut self, sample_rate: u32, freq: f32, q: f32, gain_db: f32) {
        self.coefficients = Coefficients::peak(sample_rate, freq, q, gain_db);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    /// Steady-state gain of a sine through a filter, transient discarded.
    ///
    /// Measured against the input's own sampled peak: a frequency that divides
    /// the sample rate (like 8 kHz at 48 kHz) never lands a sample on the
    /// analogue crest, so absolute peaks would under-read.
    fn response_at(filter: &mut BiquadFilter, freq: f32) -> f32 {
        let frames = (RATE as usize / 10).max(4096);
        let mut in_peak = 0.0f32;
        let mut out_peak = 0.0f32;
        for i in 0..frames {
            let x = (i as f32 * 2.0 * std::f32::consts::PI * freq / RATE as f32).sin();
            let y = filter.process(x);
            if i > frames / 2 {
                in_peak = in_peak.max(x.abs());
                out_peak = out_peak.max(y.abs());
            }
        }
        out_peak / in_peak
    }

    #[test]
    fn a_flat_shelf_is_transparent() {
        // The bug this file was rewritten for: shelves at 0 dB cut the signal
        // roughly in half each. Flat must mean flat.
        let mut low = BiquadFilter::low_shelf(RATE, 200.0, 0.7, 0.0);
        let mut high = BiquadFilter::high_shelf(RATE, 8000.0, 0.7, 0.0);
        let mut peak = BiquadFilter::peak(RATE, 1000.0, 0.7, 0.0);

        for filter in [&mut low, &mut high, &mut peak] {
            let level = response_at(filter, 1000.0);
            assert!(
                (level - 1.0).abs() < 0.01,
                "0 dB filter should be unity, got {}",
                level
            );
        }
    }

    #[test]
    fn the_dc_blocker_passes_the_band_and_kills_dc() {
        let mut hp = BiquadFilter::high_pass(RATE, 20.0, 0.7);
        let level = response_at(&mut hp, 1000.0);
        assert!(
            (level - 1.0).abs() < 0.01,
            "1 kHz should pass, got {}",
            level
        );

        let mut hp = BiquadFilter::high_pass(RATE, 20.0, 0.7);
        let mut dc_tail = 0.0f32;
        for i in 0..RATE as usize {
            let y = hp.process(0.9);
            if i > (RATE as usize) / 2 {
                dc_tail = dc_tail.max(y.abs());
            }
        }
        assert!(dc_tail < 0.01, "DC should be blocked, got {}", dc_tail);
    }

    #[test]
    fn a_boost_boosts_in_band_and_leaves_the_far_band_alone() {
        let mut shelf = BiquadFilter::low_shelf(RATE, 200.0, 0.7, 6.0);
        let low = response_at(&mut shelf, 50.0);
        assert!(
            (low - 2.0).abs() < 0.15,
            "+6 dB shelf should double a 50 Hz tone, got {}x",
            low
        );

        let mut shelf = BiquadFilter::low_shelf(RATE, 200.0, 0.7, 6.0);
        let high = response_at(&mut shelf, 8000.0);
        assert!(
            (high - 1.0).abs() < 0.05,
            "8 kHz should be untouched by a low shelf, got {}x",
            high
        );
    }

    #[test]
    fn boost_and_cut_mirror() {
        let mut boost = BiquadFilter::peak(RATE, 1000.0, 0.7, 6.0);
        let mut cut = BiquadFilter::peak(RATE, 1000.0, 0.7, -6.0);

        let boosted = response_at(&mut boost, 1000.0);
        let reduced = response_at(&mut cut, 1000.0);

        assert!(
            (boosted * reduced - 1.0).abs() < 0.02,
            "+6 then -6 should round-trip to unity, got {}",
            boosted * reduced
        );
    }

    #[test]
    fn updating_coefficients_keeps_running_audio_stable() {
        let mut filter = BiquadFilter::peak(RATE, 1000.0, 0.7, 0.0);
        for i in 0..4096 {
            let x = (i as f32 * 0.1309).sin();
            filter.process(x);
        }
        filter.update_peak_coeffs(RATE, 1000.0, 0.7, 6.0);
        for i in 0..4096 {
            let x = (i as f32 * 0.1309).sin();
            let y = filter.process(x);
            assert!(
                y.is_finite() && y.abs() < 4.0,
                "unstable after update: {}",
                y
            );
        }
    }
}
