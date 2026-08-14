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
