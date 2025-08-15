use sendin_beats_lib::audio::{VirtualMixer, MixerConfig, AudioChannel};
use tokio_test;

/// Test configuration validation functionality
#[cfg(test)]
mod config_validation_tests {
    use super::*;

    #[tokio::test]
    async fn test_valid_mixer_config() {
        let config = MixerConfig {
            sample_rate: 48000,
            buffer_size: 512,
            channels: vec![],
            master_gain: 1.0,
            master_output_device_id: None,
            monitor_output_device_id: None,
            enable_loopback: true,
        };

        let result = VirtualMixer::new(config).await;
        assert!(result.is_ok(), "Valid config should create mixer successfully");
    }

    #[tokio::test]
    async fn test_invalid_sample_rate_too_low() {
        let config = MixerConfig {
            sample_rate: 4000, // Too low
            buffer_size: 512,
            channels: vec![],
            master_gain: 1.0,
            master_output_device_id: None,
            monitor_output_device_id: None,
            enable_loopback: true,
        };

        let result = VirtualMixer::new(config).await;
        assert!(result.is_err(), "Sample rate too low should fail validation");
        assert!(result.unwrap_err().to_string().contains("Invalid sample rate"));
    }

    #[tokio::test]
    async fn test_invalid_sample_rate_too_high() {
        let config = MixerConfig {
            sample_rate: 200000, // Too high
            buffer_size: 512,
            channels: vec![],
            master_gain: 1.0,
            master_output_device_id: None,
            monitor_output_device_id: None,
            enable_loopback: true,
        };

        let result = VirtualMixer::new(config).await;
        assert!(result.is_err(), "Sample rate too high should fail validation");
        assert!(result.unwrap_err().to_string().contains("Invalid sample rate"));
    }

    #[tokio::test]
    async fn test_invalid_buffer_size_too_small() {
        let config = MixerConfig {
            sample_rate: 48000,
            buffer_size: 8, // Too small
            channels: vec![],
            master_gain: 1.0,
            master_output_device_id: None,
            monitor_output_device_id: None,
            enable_loopback: true,
        };

        let result = VirtualMixer::new(config).await;
        assert!(result.is_err(), "Buffer size too small should fail validation");
        assert!(result.unwrap_err().to_string().contains("Invalid buffer size"));
    }

    #[tokio::test]
    async fn test_invalid_buffer_size_too_large() {
        let config = MixerConfig {
            sample_rate: 48000,
            buffer_size: 16384, // Too large
            channels: vec![],
            master_gain: 1.0,
            master_output_device_id: None,
            monitor_output_device_id: None,
            enable_loopback: true,
        };

        let result = VirtualMixer::new(config).await;
        assert!(result.is_err(), "Buffer size too large should fail validation");
        assert!(result.unwrap_err().to_string().contains("Invalid buffer size"));
    }

    #[tokio::test]
    async fn test_invalid_master_gain_negative() {
        let config = MixerConfig {
            sample_rate: 48000,
            buffer_size: 512,
            channels: vec![],
            master_gain: -0.5, // Negative gain
            master_output_device_id: None,
            monitor_output_device_id: None,
            enable_loopback: true,
        };

        let result = VirtualMixer::new(config).await;
        assert!(result.is_err(), "Negative master gain should fail validation");
        assert!(result.unwrap_err().to_string().contains("Invalid master gain"));
    }

    #[tokio::test]
    async fn test_invalid_master_gain_too_high() {
        let config = MixerConfig {
            sample_rate: 48000,
            buffer_size: 512,
            channels: vec![],
            master_gain: 5.0, // Too high
            master_output_device_id: None,
            monitor_output_device_id: None,
            enable_loopback: true,
        };

        let result = VirtualMixer::new(config).await;
        assert!(result.is_err(), "Master gain too high should fail validation");
        assert!(result.unwrap_err().to_string().contains("Invalid master gain"));
    }

    #[tokio::test]
    async fn test_too_many_channels() {
        let mut channels = Vec::new();
        for i in 0..35 {
            channels.push(AudioChannel {
                id: i,
                name: format!("Channel {}", i),
                ..Default::default()
            });
        }

        let config = MixerConfig {
            sample_rate: 48000,
            buffer_size: 512,
            channels,
            master_gain: 1.0,
            master_output_device_id: None,
            monitor_output_device_id: None,
            enable_loopback: true,
        };

        let result = VirtualMixer::new(config).await;
        assert!(result.is_err(), "Too many channels should fail validation");
        assert!(result.unwrap_err().to_string().contains("Too many channels"));
    }

    #[tokio::test]
    async fn test_invalid_channel_gain() {
        let channel = AudioChannel {
            id: 1,
            name: "Test Channel".to_string(),
            gain: 5.0, // Too high
            ..Default::default()
        };

        let config = MixerConfig {
            sample_rate: 48000,
            buffer_size: 512,
            channels: vec![channel],
            master_gain: 1.0,
            master_output_device_id: None,
            monitor_output_device_id: None,
            enable_loopback: true,
        };

        let result = VirtualMixer::new(config).await;
        assert!(result.is_err(), "Invalid channel gain should fail validation");
        assert!(result.unwrap_err().to_string().contains("Invalid gain for channel"));
    }

    #[tokio::test]
    async fn test_invalid_channel_pan() {
        let channel = AudioChannel {
            id: 1,
            name: "Test Channel".to_string(),
            pan: 2.0, // Outside range
            ..Default::default()
        };

        let config = MixerConfig {
            sample_rate: 48000,
            buffer_size: 512,
            channels: vec![channel],
            master_gain: 1.0,
            master_output_device_id: None,
            monitor_output_device_id: None,
            enable_loopback: true,
        };

        let result = VirtualMixer::new(config).await;
        assert!(result.is_err(), "Invalid channel pan should fail validation");
        assert!(result.unwrap_err().to_string().contains("Invalid pan for channel"));
    }

    #[tokio::test]
    async fn test_invalid_eq_gain() {
        let channel = AudioChannel {
            id: 1,
            name: "Test Channel".to_string(),
            eq_low_gain: 30.0, // Too high
            ..Default::default()
        };

        let config = MixerConfig {
            sample_rate: 48000,
            buffer_size: 512,
            channels: vec![channel],
            master_gain: 1.0,
            master_output_device_id: None,
            monitor_output_device_id: None,
            enable_loopback: true,
        };

        let result = VirtualMixer::new(config).await;
        assert!(result.is_err(), "Invalid EQ gain should fail validation");
        assert!(result.unwrap_err().to_string().contains("Invalid EQ low gain"));
    }
}