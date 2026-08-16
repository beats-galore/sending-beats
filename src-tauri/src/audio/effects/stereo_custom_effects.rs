// Stereo wrapper over the mono custom effects chain.
//
// Every stage of `CustomAudioEffectsChain` carries per-sample state — biquad
// memory, compressor and limiter envelopes, the limiter's lookahead delay
// line. Feeding interleaved stereo through one chain alternates left and right
// samples through that shared state, which comb-filters the EQ and makes the
// limiter's delay line swap channels outright. So a stereo signal gets one
// chain per side: the block is split into per-channel scratch buffers, each
// side is processed by its own chain, and the result is interleaved back.
//
// The two sides are deliberately dual-mono — each dynamics detector follows
// its own channel. Heavily one-sided material can therefore shift the stereo
// image under compression; a linked detector is the upgrade path if that ever
// matters in practice.

use super::custom_effects_chain::CustomAudioEffectsChain;
use super::equalizer::EQBand;

/// Every setting the chain holds, in one place.
///
/// This is what travels from a persisted effects row into a worker at attach,
/// and what lets the wrapper rebuild its chains at a new sample rate without
/// losing the knobs.
#[derive(Debug, Clone, Copy)]
pub struct ChainSettings {
    pub eq_low_gain_db: f32,
    pub eq_mid_gain_db: f32,
    pub eq_high_gain_db: f32,
    pub comp_threshold_db: f32,
    pub comp_ratio: f32,
    pub comp_attack_ms: f32,
    pub comp_release_ms: f32,
    pub comp_enabled: bool,
    pub limiter_threshold_db: f32,
    pub limiter_enabled: bool,
}

impl From<&crate::entities::audio_effects_default::Model> for ChainSettings {
    fn from(row: &crate::entities::audio_effects_default::Model) -> Self {
        Self {
            eq_low_gain_db: row.eq_low_gain,
            eq_mid_gain_db: row.eq_mid_gain,
            eq_high_gain_db: row.eq_high_gain,
            comp_threshold_db: row.comp_threshold,
            comp_ratio: row.comp_ratio,
            comp_attack_ms: row.comp_attack,
            comp_release_ms: row.comp_release,
            comp_enabled: row.comp_enabled,
            limiter_threshold_db: row.limiter_threshold,
            limiter_enabled: row.limiter_enabled,
        }
    }
}

/// How a channel strip was last left: the fader state the default chain
/// applies, and the settings the custom chain runs with.
///
/// This is what a persisted effects row becomes on its way into a worker, so
/// the pipeline doesn't handle database models directly.
#[derive(Debug, Clone, Copy)]
pub struct ChannelStripState {
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub chain: ChainSettings,
}

impl From<&crate::entities::audio_effects_default::Model> for ChannelStripState {
    fn from(row: &crate::entities::audio_effects_default::Model) -> Self {
        Self {
            gain: row.gain,
            pan: row.pan,
            muted: row.muted,
            solo: row.solo,
            chain: ChainSettings::from(row),
        }
    }
}

impl Default for ChainSettings {
    fn default() -> Self {
        // Matches what a freshly constructed chain does: EQ flat, dynamics
        // bypassed, thresholds at the DSP constructors' own defaults.
        Self {
            eq_low_gain_db: 0.0,
            eq_mid_gain_db: 0.0,
            eq_high_gain_db: 0.0,
            comp_threshold_db: -12.0,
            comp_ratio: 4.0,
            comp_attack_ms: 10.0,
            comp_release_ms: 200.0,
            comp_enabled: false,
            limiter_threshold_db: -0.1,
            limiter_enabled: false,
        }
    }
}

/// One custom chain per stereo side, plus the scratch space to split a block.
#[derive(Debug)]
pub struct StereoCustomEffects {
    left: CustomAudioEffectsChain,
    right: CustomAudioEffectsChain,
    sample_rate: u32,
    enabled: bool,
    /// The applied settings, kept so a sample-rate rebuild is lossless
    settings: ChainSettings,
    left_scratch: Vec<f32>,
    right_scratch: Vec<f32>,
}

impl StereoCustomEffects {
    pub fn new(sample_rate: u32) -> Self {
        Self::with_settings(sample_rate, ChainSettings::default())
    }

    pub fn with_settings(sample_rate: u32, settings: ChainSettings) -> Self {
        let mut this = Self {
            left: CustomAudioEffectsChain::new(sample_rate),
            right: CustomAudioEffectsChain::new(sample_rate),
            sample_rate,
            enabled: false,
            settings,
            left_scratch: Vec::new(),
            right_scratch: Vec::new(),
        };
        this.apply_settings();
        this
    }

    /// Push every cached setting into both chains.
    fn apply_settings(&mut self) {
        let s = self.settings;
        for chain in [&mut self.left, &mut self.right] {
            chain.set_eq_gain(EQBand::Low, s.eq_low_gain_db);
            chain.set_eq_gain(EQBand::Mid, s.eq_mid_gain_db);
            chain.set_eq_gain(EQBand::High, s.eq_high_gain_db);
            chain.set_compressor_params(
                s.comp_threshold_db,
                s.comp_ratio,
                s.comp_attack_ms,
                s.comp_release_ms,
            );
            chain.set_compressor_enabled(s.comp_enabled);
            chain.set_limiter_threshold(s.limiter_threshold_db);
            chain.set_limiter_enabled(s.limiter_enabled);
            chain.set_enabled(self.enabled);
        }
    }

    /// Process one interleaved stereo block in place.
    ///
    /// The scratch buffers grow to half a block once and are reused, so steady
    /// state allocates nothing.
    pub fn process_stereo_interleaved(&mut self, samples: &mut [f32]) {
        if !self.enabled {
            return;
        }

        let frames = samples.len() / 2;
        self.left_scratch.clear();
        self.right_scratch.clear();
        self.left_scratch.reserve(frames);
        self.right_scratch.reserve(frames);

        for frame in samples.chunks_exact(2) {
            self.left_scratch.push(frame[0]);
            self.right_scratch.push(frame[1]);
        }

        self.left.process(&mut self.left_scratch);
        self.right.process(&mut self.right_scratch);

        for (i, frame) in samples.chunks_exact_mut(2).enumerate() {
            frame[0] = self.left_scratch[i];
            frame[1] = self.right_scratch[i];
        }
    }

    /// Switch the whole chain on or off. Off is a true bypass.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.left.set_enabled(enabled);
        self.right.set_enabled(enabled);
    }

    pub fn set_eq_gain(&mut self, band: EQBand, gain_db: f32) {
        match band {
            EQBand::Low => self.settings.eq_low_gain_db = gain_db,
            EQBand::Mid => self.settings.eq_mid_gain_db = gain_db,
            EQBand::High => self.settings.eq_high_gain_db = gain_db,
        }
        self.left.set_eq_gain(band, gain_db);
        self.right.set_eq_gain(band, gain_db);
    }

    /// Update whichever compressor parameters were provided.
    pub fn update_compressor(
        &mut self,
        threshold_db: Option<f32>,
        ratio: Option<f32>,
        attack_ms: Option<f32>,
        release_ms: Option<f32>,
        enabled: Option<bool>,
    ) {
        let s = &mut self.settings;
        s.comp_threshold_db = threshold_db.unwrap_or(s.comp_threshold_db);
        s.comp_ratio = ratio.unwrap_or(s.comp_ratio);
        s.comp_attack_ms = attack_ms.unwrap_or(s.comp_attack_ms);
        s.comp_release_ms = release_ms.unwrap_or(s.comp_release_ms);
        if let Some(enabled) = enabled {
            s.comp_enabled = enabled;
        }

        let s = self.settings;
        for chain in [&mut self.left, &mut self.right] {
            chain.set_compressor_params(
                s.comp_threshold_db,
                s.comp_ratio,
                s.comp_attack_ms,
                s.comp_release_ms,
            );
            chain.set_compressor_enabled(s.comp_enabled);
        }
    }

    /// Update whichever limiter parameters were provided.
    pub fn update_limiter(&mut self, threshold_db: Option<f32>, enabled: Option<bool>) {
        let s = &mut self.settings;
        s.limiter_threshold_db = threshold_db.unwrap_or(s.limiter_threshold_db);
        if let Some(enabled) = enabled {
            s.limiter_enabled = enabled;
        }

        let s = self.settings;
        for chain in [&mut self.left, &mut self.right] {
            chain.set_limiter_threshold(s.limiter_threshold_db);
            chain.set_limiter_enabled(s.limiter_enabled);
        }
    }

    /// Rebuild both chains at a new rate, keeping every setting.
    ///
    /// Filter coefficients and envelope timings are derived from the sample
    /// rate, so the chains cannot simply be kept — but the knobs can.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        if sample_rate == self.sample_rate {
            return;
        }
        self.sample_rate = sample_rate;
        self.left = CustomAudioEffectsChain::new(sample_rate);
        self.right = CustomAudioEffectsChain::new(sample_rate);
        self.apply_settings();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn enabled_chain() -> StereoCustomEffects {
        let mut fx = StereoCustomEffects::new(RATE);
        fx.set_enabled(true);
        fx
    }

    #[test]
    fn a_disabled_chain_is_a_true_bypass() {
        let mut fx = StereoCustomEffects::new(RATE);
        let mut samples = vec![0.5, -0.25, 0.1, 0.9];
        let original = samples.clone();

        fx.process_stereo_interleaved(&mut samples);

        assert_eq!(samples, original);
    }

    /// The reason this wrapper exists: one chain's filter state must never see
    /// the other channel's samples.
    #[test]
    fn a_signal_on_one_side_stays_on_that_side() {
        let mut fx = enabled_chain();
        fx.set_eq_gain(EQBand::Low, 6.0);

        // Left carries a signal, right is silent
        let mut samples: Vec<f32> = (0..256)
            .flat_map(|i| [((i as f32) * 0.05).sin() * 0.5, 0.0])
            .collect();

        fx.process_stereo_interleaved(&mut samples);

        let right_energy: f32 = samples.iter().skip(1).step_by(2).map(|s| s * s).sum();
        let left_energy: f32 = samples.iter().step_by(2).map(|s| s * s).sum();

        assert!(left_energy > 0.0, "left should still carry the signal");
        assert_eq!(right_energy, 0.0, "silence in must be silence out");
    }

    /// A hot in-band tone, well inside the DC blocker's passband.
    fn sine_block(frames: usize, amplitude: f32) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let s = (i as f32 * 2.0 * std::f32::consts::PI * 1000.0 / RATE as f32).sin()
                    * amplitude;
                [s, s]
            })
            .collect()
    }

    /// Peak level over the last quarter of a block, once envelopes have settled.
    fn tail_peak(samples: &[f32]) -> f32 {
        samples[samples.len() * 3 / 4..]
            .iter()
            .fold(0.0f32, |peak, s| peak.max(s.abs()))
    }

    #[test]
    fn bypassed_dynamics_leave_a_hot_signal_alone() {
        let mut fx = enabled_chain();
        // Compressor and limiter default to bypassed; a tone over their
        // thresholds must come through with only DC-blocker/EQ shaping.
        let mut samples = sine_block(2048, 0.9);
        fx.process_stereo_interleaved(&mut samples);

        let tail = tail_peak(&samples);
        assert!(tail > 0.6, "dynamics should be bypassed, got {}", tail);
    }

    #[test]
    fn engaging_the_compressor_reduces_a_hot_signal() {
        let mut with = enabled_chain();
        with.update_compressor(Some(-24.0), Some(8.0), Some(0.1), Some(200.0), Some(true));

        let mut without = enabled_chain();

        let mut compressed = sine_block(4096, 0.9);
        let mut untouched = sine_block(4096, 0.9);

        with.process_stereo_interleaved(&mut compressed);
        without.process_stereo_interleaved(&mut untouched);

        let tail_c = tail_peak(&compressed);
        let tail_u = tail_peak(&untouched);
        assert!(
            tail_c < tail_u * 0.7,
            "compressed tail {} should sit well under bypassed tail {}",
            tail_c,
            tail_u
        );
    }

    #[test]
    fn a_sample_rate_change_keeps_the_knobs() {
        let mut fx = enabled_chain();
        fx.set_eq_gain(EQBand::High, -6.0);
        fx.update_compressor(Some(-24.0), Some(8.0), Some(0.1), None, Some(true));

        fx.set_sample_rate(44_100);

        // The compressor must still be engaged after the rebuild
        let mut compressed = sine_block(4096, 0.9);
        fx.process_stereo_interleaved(&mut compressed);

        let mut plain = enabled_chain();
        plain.set_sample_rate(44_100);
        let mut untouched = sine_block(4096, 0.9);
        plain.process_stereo_interleaved(&mut untouched);

        assert!(
            tail_peak(&compressed) < tail_peak(&untouched) * 0.7,
            "settings should survive the rebuild"
        );
    }
}
