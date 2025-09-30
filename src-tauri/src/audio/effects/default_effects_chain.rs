use colored::*;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct DefaultAudioEffectsChain {
    gain: f32,
    pan: f32,
    muted: bool,
    solo: bool,
    device_id: String,
}

impl DefaultAudioEffectsChain {
    pub fn new(device_id: String) -> Self {
        Self {
            gain: 1.0,
            pan: 0.0,
            muted: false,
            solo: false,
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

    pub fn process_stereo(&self, left: &mut [f32], right: &mut [f32], any_channel_solo: bool) {
        if self.muted || (any_channel_solo && !self.solo) {
            left.fill(0.0);
            right.fill(0.0);
            return;
        }

        let left_gain = self.gain * if self.pan <= 0.0 { 1.0 } else { 1.0 - self.pan };
        let right_gain = self.gain * if self.pan >= 0.0 { 1.0 } else { 1.0 + self.pan };

        for sample in left.iter_mut() {
            *sample *= left_gain;
        }

        for sample in right.iter_mut() {
            *sample *= right_gain;
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
