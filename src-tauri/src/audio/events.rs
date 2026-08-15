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

/// Levels of one bus, as its outputs receive it
///
/// Keyed by bus id rather than the numeric channel `VULevelEvent` uses, which a
/// bus has no equivalent of. This is measured after the bus's own gain, so it is
/// what the outputs taking it are actually being handed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusVULevelEvent {
    pub bus_id: String,
    /// Peak level for the left channel (-∞ to 0 dB)
    pub peak_left: f32,
    /// Peak level for the right channel (-∞ to 0 dB)
    pub peak_right: f32,
    /// RMS level for the left channel (-∞ to 0 dB)
    pub rms_left: f32,
    /// RMS level for the right channel (-∞ to 0 dB)
    pub rms_right: f32,
    /// Timestamp in microseconds since Unix epoch
    pub timestamp: u64,
}

impl BusVULevelEvent {
    pub fn new(
        bus_id: String,
        peak_left: f32,
        peak_right: f32,
        rms_left: f32,
        rms_right: f32,
    ) -> Self {
        Self {
            bus_id,
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
/// This enum allows sending channel, bus and master data through a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum VUChannelData {
    Channel(VULevelEvent),
    Bus(BusVULevelEvent),
    Master(MasterVULevelEvent),
}

impl VUChannelData {
    pub fn from_channel(event: VULevelEvent) -> Self {
        Self::Channel(event)
    }

    pub fn from_bus(event: BusVULevelEvent) -> Self {
        Self::Bus(event)
    }

    pub fn from_master(event: MasterVULevelEvent) -> Self {
        Self::Master(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frontend switches on `type` and reads `data`, so the tagging is a
    /// contract rather than an implementation detail
    #[test]
    fn bus_levels_are_tagged_for_the_frontend() {
        let event = VUChannelData::from_bus(BusVULevelEvent::new(
            "cue".to_string(),
            -6.0,
            -6.5,
            -12.0,
            -12.5,
        ));

        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["type"], "Bus");
        assert_eq!(json["data"]["bus_id"], "cue");
        assert_eq!(json["data"]["peak_left"], -6.0);
        assert_eq!(json["data"]["peak_right"], -6.5);
        assert_eq!(json["data"]["rms_left"], -12.0);
        assert_eq!(json["data"]["rms_right"], -12.5);
    }

    #[test]
    fn the_existing_variants_keep_their_tags() {
        // Adding Bus must not renumber or rename what the frontend already reads
        let channel = VUChannelData::from_channel(VULevelEvent::new(
            "channel_0".to_string(),
            0,
            -3.0,
            -3.0,
            -9.0,
            -9.0,
            true,
        ));
        let master = VUChannelData::from_master(MasterVULevelEvent::new(-1.0, -1.0, -7.0, -7.0));

        assert_eq!(serde_json::to_value(&channel).unwrap()["type"], "Channel");
        assert_eq!(serde_json::to_value(&master).unwrap()["type"], "Master");
    }
}
