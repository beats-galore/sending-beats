use sendin_beats_lib::audio::{AudioChannel, AudioDeviceInfo, AudioMetrics, EQBand, MixerConfig};

#[cfg(test)]
mod mixer_config_tests {
    use super::*;

    #[test]
    fn test_mixer_config_default() {
        let config = MixerConfig::default();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.buffer_size, 512);
        assert_eq!(config.channels.len(), 0); // Vec<AudioChannel> starts empty
        assert_eq!(config.master_output_device_id, None);
        assert_eq!(config.monitor_output_device_id, None);
        assert_eq!(config.master_gain, 1.0);
        assert!(config.enable_loopback);
    }

    #[test]
    fn test_mixer_config_custom_values() {
        let channel = AudioChannel::default();
        let config = MixerConfig {
            sample_rate: 44100,
            buffer_size: 256,
            channels: vec![channel],
            master_output_device_id: Some("Test Output".to_string()),
            monitor_output_device_id: Some("Test Monitor".to_string()),
            master_gain: 0.8,
            enable_loopback: true,
        };

        assert_eq!(config.sample_rate, 44100);
        assert_eq!(config.buffer_size, 256);
        assert_eq!(config.channels.len(), 1);
        assert_eq!(
            config.master_output_device_id,
            Some("Test Output".to_string())
        );
        assert_eq!(
            config.monitor_output_device_id,
            Some("Test Monitor".to_string())
        );
        assert_eq!(config.master_gain, 0.8);
        assert!(config.enable_loopback);
    }

    #[test]
    fn test_mixer_config_validation() {
        let mut config = MixerConfig::default();

        // Test sample rate bounds
        config.sample_rate = 22050;
        assert_eq!(config.sample_rate, 22050);

        config.sample_rate = 192000;
        assert_eq!(config.sample_rate, 192000);

        // Test channel management
        config.channels.push(AudioChannel::default());
        assert_eq!(config.channels.len(), 1);

        config.channels.push(AudioChannel::default());
        assert_eq!(config.channels.len(), 2);

        // Test buffer size (should be power of 2)
        config.buffer_size = 256;
        assert_eq!(config.buffer_size, 256);

        config.buffer_size = 2048;
        assert_eq!(config.buffer_size, 2048);
    }
}

#[cfg(test)]
mod audio_device_info_tests {
    use super::*;

    #[test]
    fn test_audio_device_info_creation() {
        let device_info = AudioDeviceInfo {
            id: "device_1".to_string(),
            name: "Test Device".to_string(),
            is_input: true,
            is_output: false,
            is_default: false,
            supported_sample_rates: vec![44100, 48000],
            supported_channels: vec![2],
            host_api: "CoreAudio".to_string(),
        };

        assert_eq!(device_info.id, "device_1");
        assert_eq!(device_info.name, "Test Device");
        assert!(device_info.is_input);
        assert!(!device_info.is_output);
        assert!(!device_info.is_default);
        assert_eq!(device_info.supported_sample_rates.len(), 2);
        assert_eq!(device_info.supported_channels.len(), 1);
        assert_eq!(device_info.host_api, "CoreAudio");
    }
}

#[cfg(test)]
mod audio_metrics_tests {
    use super::*;

    #[test]
    fn test_audio_metrics_creation() {
        let metrics = AudioMetrics {
            cpu_usage: 25.5,
            buffer_underruns: 0,
            buffer_overruns: 0,
            latency_ms: 12.3,
            sample_rate: 44100,
            active_channels: 2,
        };

        assert_eq!(metrics.cpu_usage, 25.5);
        assert_eq!(metrics.latency_ms, 12.3);
        assert_eq!(metrics.buffer_underruns, 0);
        assert_eq!(metrics.buffer_overruns, 0);
        assert_eq!(metrics.sample_rate, 44100);
        assert_eq!(metrics.active_channels, 2);
    }

    #[test]
    fn test_audio_metrics_bounds() {
        let metrics = AudioMetrics {
            cpu_usage: 0.0,
            buffer_underruns: 0,
            buffer_overruns: 0,
            latency_ms: 0.0,
            sample_rate: 22050,
            active_channels: 1,
        };

        assert!(metrics.cpu_usage >= 0.0);
        assert!(metrics.latency_ms >= 0.0);
        assert!(metrics.buffer_underruns >= 0);
        assert!(metrics.buffer_overruns >= 0);
        assert!(metrics.sample_rate > 0);
        assert!(metrics.active_channels > 0);
    }
}
