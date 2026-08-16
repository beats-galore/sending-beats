use colored::*;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct DefaultAudioEffectsChain {
    gain: f32,
    pan: f32,
    muted: bool,
    solo: bool,
    /// Whether the channel's effects are switched on. Only pan answers to this —
    /// gain, mute and solo are routing and apply whatever the effects are doing.
    effects_enabled: bool,
    device_id: String,
}

impl DefaultAudioEffectsChain {
    pub fn new(device_id: String) -> Self {
        Self {
            gain: 1.0,
            pan: 0.0,
            muted: false,
            solo: false,
            effects_enabled: false,
            device_id,
        }
    }

    pub fn set_gain(&mut self, gain_linear: f32) {
        self.gain = gain_linear.max(0.0);
        debug!(
            "{}: Set gain to {:.2} ({:.1}dB) for device {}",
            "DEFAULT_FX".on_cyan().white(),
            self.gain,
            20.0 * self.gain.log10(),
            self.device_id
        );
    }

    pub fn set_pan(&mut self, pan: f32) {
        self.pan = pan.clamp(-1.0, 1.0);
        debug!(
            "{}: Set pan to {:.2} for device {}",
            "DEFAULT_FX".on_cyan().white(),
            self.pan,
            self.device_id
        );
    }

    pub fn set_effects_enabled(&mut self, enabled: bool) {
        self.effects_enabled = enabled;
        debug!(
            "{}: Effects {} for device {}",
            "DEFAULT_FX".on_cyan().white(),
            if enabled { "enabled" } else { "disabled" },
            self.device_id
        );
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        debug!(
            "{}: {} device {}",
            "DEFAULT_FX".on_cyan().white(),
            if muted { "Muted" } else { "Unmuted" },
            self.device_id
        );
    }

    pub fn set_solo(&mut self, solo: bool) {
        self.solo = solo;
        debug!(
            "{}: {} solo for device {}",
            "DEFAULT_FX".on_cyan().white(),
            if solo { "Enabled" } else { "Disabled" },
            self.device_id
        );
    }

    pub fn is_solo(&self) -> bool {
        self.solo
    }

    pub fn process_stereo_interleaved(&self, samples: &mut [f32], any_channel_solo: bool) {
        if self.muted || (any_channel_solo && !self.solo) {
            samples.fill(0.0);
            return;
        }

        // Pan is part of the effects chain, so it sits centred while the chain is
        // switched off rather than placing the channel from a control the
        // interface is not showing.
        let pan = if self.effects_enabled { self.pan } else { 0.0 };

        let left_gain = self.gain * if pan <= 0.0 { 1.0 } else { 1.0 - pan };
        let right_gain = self.gain * if pan >= 0.0 { 1.0 } else { 1.0 + pan };

        for i in 0..(samples.len() / 2) {
            samples[i * 2] *= left_gain;
            samples[i * 2 + 1] *= right_gain;
        }
    }

    pub fn process_mono(&self, samples: &mut [f32], any_channel_solo: bool) {
        if self.muted || (any_channel_solo && !self.solo) {
            samples.fill(0.0);
            return;
        }

        for sample in samples.iter_mut() {
            *sample *= self.gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DefaultAudioEffectsChain;

    fn chain(id: &str) -> DefaultAudioEffectsChain {
        DefaultAudioEffectsChain::new(id.to_string())
    }

    /// Whether anything in the mix is soloed
    ///
    /// The rule the pipeline applies. Written out here because it is the rule
    /// that was wrong: the shared flag used to be assigned whichever channel
    /// was toggled last rather than asked of all of them.
    fn any_solo(chains: &[&DefaultAudioEffectsChain]) -> bool {
        chains.iter().any(|chain| chain.is_solo())
    }

    fn silenced(chain: &DefaultAudioEffectsChain, any_solo: bool) -> bool {
        let mut samples = vec![1.0_f32; 4];
        chain.process_stereo_interleaved(&mut samples, any_solo);
        samples.iter().all(|sample| *sample == 0.0)
    }

    #[test]
    fn soloing_one_channel_silences_the_others() {
        let mut lead = chain("lead");
        let other = chain("other");
        lead.set_solo(true);

        let any = any_solo(&[&lead, &other]);
        assert!(any);
        assert!(!silenced(&lead, any));
        assert!(silenced(&other, any));
    }

    /// The bug from #120
    ///
    /// Solo two channels, un-solo the second: the first is still soloed, so
    /// everything else must stay silent. The shared flag used to be set to the
    /// second channel's new value — false — which un-muted the whole mix while
    /// the first channel was still lit in the interface.
    #[test]
    fn un_soloing_one_of_two_leaves_the_other_soloed() {
        let mut first = chain("first");
        let mut second = chain("second");
        let other = chain("other");

        first.set_solo(true);
        second.set_solo(true);
        second.set_solo(false);

        let any = any_solo(&[&first, &second, &other]);

        assert!(any, "a channel is still soloed");
        assert!(silenced(&other, any), "a channel with no solo stays silent");
        assert!(
            silenced(&second, any),
            "the un-soloed channel is silent too"
        );
        assert!(
            !silenced(&first, any),
            "the channel still soloed is audible"
        );
    }

    #[test]
    fn un_soloing_the_last_one_brings_everything_back() {
        let mut lead = chain("lead");
        let other = chain("other");

        lead.set_solo(true);
        lead.set_solo(false);

        let any = any_solo(&[&lead, &other]);

        assert!(!any);
        assert!(!silenced(&other, any));
        assert!(!silenced(&lead, any));
    }

    /// Mute outranks solo: a soloed channel that is also muted stays silent
    #[test]
    fn a_muted_channel_is_silent_even_when_soloed() {
        let mut lead = chain("lead");
        lead.set_solo(true);
        lead.set_muted(true);

        assert!(silenced(&lead, true));
    }
}
