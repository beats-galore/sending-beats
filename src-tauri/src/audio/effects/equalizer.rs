use super::filter::BiquadFilter;

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
                self.low_shelf
                    .update_low_shelf_coeffs(self.sample_rate, 200.0, 0.7, gain_db);
            }
            EQBand::Mid => {
                self.mid_peak
                    .update_peak_coeffs(self.sample_rate, 1000.0, 0.7, gain_db);
            }
            EQBand::High => {
                self.high_shelf
                    .update_high_shelf_coeffs(self.sample_rate, 8000.0, 0.7, gain_db);
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
