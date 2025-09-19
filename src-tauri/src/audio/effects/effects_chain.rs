use anyhow::Result;

use super::compressor::Compressor;
use super::equalizer::{EQBand, ThreeBandEqualizer};
use super::filter::BiquadFilter;
use super::limiter::Limiter;

/// Real-time audio effects chain
#[derive(Debug)]
pub struct AudioEffectsChain {
    dc_blocker: BiquadFilter, // **BASS POPPING FIX**: DC offset removal
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
            enabled: false,
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

    pub fn set_compressor_params(
        &mut self,
        threshold: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
    ) {
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
        self.dc_blocker.reset(); // **BASS POPPING FIX**: Reset DC blocker too
        self.equalizer.reset();
        self.compressor.reset();
        self.limiter.reset();
    }
}
