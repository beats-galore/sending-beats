/// Trait for VU meter processing services
/// Allows unified interface for both event-based and channel-based VU processing
pub trait VUProcessor: Send {
    /// Process channel audio and emit/send VU level data
    /// channel_samples: Interleaved stereo samples [L, R, L, R, ...]
    fn process_channel_audio(&mut self, channel_id: u32, channel_samples: &[f32]);

    /// Process master output audio and emit/send master VU level data
    /// master_samples: Interleaved stereo samples [L, R, L, R, ...]
    fn process_master_audio(&mut self, master_samples: &[f32]);
}

// Implement the trait for VULevelService (event-based)
impl VUProcessor for crate::audio::vu_service::VULevelService {
    fn process_channel_audio(&mut self, channel_id: u32, channel_samples: &[f32]) {
        self.process_channel_audio(channel_id, channel_samples);
    }

    fn process_master_audio(&mut self, master_samples: &[f32]) {
        self.process_master_audio(master_samples);
    }
}

// Implement the trait for VUChannelService (channel-based)
impl VUProcessor for crate::audio::vu_channel_service::VUChannelService {
    fn process_channel_audio(&mut self, channel_id: u32, channel_samples: &[f32]) {
        self.process_channel_audio(channel_id, channel_samples);
    }

    fn process_master_audio(&mut self, master_samples: &[f32]) {
        self.process_master_audio(master_samples);
    }
}