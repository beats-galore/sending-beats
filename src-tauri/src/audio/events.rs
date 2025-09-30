use serde::{Deserialize, Serialize};

/// Real-time VU meter level data for event emission
/// Lightweight, fire-and-forget data structure for immediate UI updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VULevelEvent {
    /// Device identifier (could be device name or channel ID)
    pub device_id: String,
    /// Channel number (0-based)
    pub channel: u32,
    /// Peak level for left channel (-∞ to 0 dB)
    pub peak_left: f32,
    /// Peak level for right channel (-∞ to 0 dB)
    pub peak_right: f32,
    /// RMS level for left channel (-∞ to 0 dB)
    pub rms_left: f32,
    /// RMS level for right channel (-∞ to 0 dB)
    pub rms_right: f32,
    /// Whether this is stereo data
    pub is_stereo: bool,
    /// Timestamp in microseconds since Unix epoch
    pub timestamp: u64,
}

impl VULevelEvent {
    /// Create new VU level event for immediate emission
    pub fn new(
        device_id: String,
        channel: u32,
        peak_left: f32,
        peak_right: f32,
        rms_left: f32,
        rms_right: f32,
        is_stereo: bool,
    ) -> Self {
        Self {
            device_id,
            channel,
            peak_left,
            peak_right,
            rms_left,
            rms_right,
            is_stereo,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64,
        }
    }

    /// Create mono VU level event
    pub fn new_mono(device_id: String, channel: u32, peak: f32, rms: f32) -> Self {
        Self::new(device_id, channel, peak, 0.0, rms, 0.0, false)
    }
}

/// Master output VU levels (separate from channel levels)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterVULevelEvent {
    /// Peak level for left master channel (-∞ to 0 dB)
    pub peak_left: f32,
    /// Peak level for right master channel (-∞ to 0 dB)
    pub peak_right: f32,
    /// RMS level for left master channel (-∞ to 0 dB)
    pub rms_left: f32,
    /// RMS level for right master channel (-∞ to 0 dB)
    pub rms_right: f32,
    /// Timestamp in microseconds since Unix epoch
    pub timestamp: u64,
}

impl MasterVULevelEvent {
    pub fn new(peak_left: f32, peak_right: f32, rms_left: f32, rms_right: f32) -> Self {
        Self {
            peak_left,
            peak_right,
            rms_left,
            rms_right,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64,
        }
    }
}

/// Combined VU data for efficient Tauri channel streaming
/// This enum allows sending both channel and master data through a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum VUChannelData {
    Channel(VULevelEvent),
    Master(MasterVULevelEvent),
}

impl VUChannelData {
    pub fn from_channel(event: VULevelEvent) -> Self {
        Self::Channel(event)
    }

    pub fn from_master(event: MasterVULevelEvent) -> Self {
        Self::Master(event)
    }
}
