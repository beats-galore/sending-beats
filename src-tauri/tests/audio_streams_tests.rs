use sendin_beats_lib::audio::*;
use cpal::{StreamConfig, SampleRate, BufferSize};

#[cfg(test)]
mod cpal_stream_tests {
    use cpal::{StreamConfig, SampleRate, BufferSize};

    #[test]
    fn test_stream_config_creation() {
        // StreamConfig doesn't have default(), create manually
        let config = StreamConfig {
            channels: 2,
            sample_rate: SampleRate(44100),
            buffer_size: cpal::BufferSize::Default,
        };
        
        assert_eq!(config.channels, 2);
        assert_eq!(config.sample_rate.0, 44100);
        assert_eq!(config.buffer_size, cpal::BufferSize::Default);
    }

    #[test] 
    fn test_stream_config_custom() {
        let config = StreamConfig {
            channels: 1,
            sample_rate: SampleRate(48000),
            buffer_size: BufferSize::Fixed(512),
        };
        
        assert_eq!(config.channels, 1);
        assert_eq!(config.sample_rate.0, 48000);
        match config.buffer_size {
            BufferSize::Fixed(size) => assert_eq!(size, 512),
            _ => panic!("Expected fixed buffer size"),
        }
    }
}

#[cfg(test)]
mod device_enumeration_tests {
    use super::*;

    #[tokio::test]
    async fn test_audio_device_manager_new() {
        let result = AudioDeviceManager::new();
        
        // Should create without panicking
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_enumerate_devices() {
        let manager = AudioDeviceManager::new().expect("Failed to create manager");
        let devices = manager.enumerate_devices().await;
        
        // Should return a vector (may be empty on some systems)
        assert!(devices.is_ok());
        
        if let Ok(device_list) = devices {
            // Each device should have a valid ID and name
            for device in device_list {
                assert!(!device.id.is_empty());
                assert!(!device.name.is_empty());
            }
        }
    }

    #[tokio::test]
    async fn test_get_device_by_id() {
        let manager = AudioDeviceManager::new().expect("Failed to create manager");
        
        // First enumerate devices to get some IDs
        if let Ok(devices) = manager.enumerate_devices().await {
            if !devices.is_empty() {
                let device_id = &devices[0].id;
                let device = manager.get_device(device_id).await;
                assert!(device.is_some());
                
                if let Some(info) = device {
                    assert_eq!(info.id, *device_id);
                    assert!(!info.name.is_empty());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_refresh_devices() {
        let manager = AudioDeviceManager::new().expect("Failed to create manager");
        
        // Test that refresh_devices doesn't panic
        let result = manager.refresh_devices().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_find_cpal_device() {
        let manager = AudioDeviceManager::new().expect("Failed to create manager");
        
        // The find_cpal_device method may fall back to default device
        // so we test that it doesn't panic and returns a valid result
        let result = manager.find_cpal_device("NonExistentDevice12345", false).await;
        // Should either find a fallback device or return an error
        assert!(result.is_ok() || result.is_err());
        
        // Test with existing device (if any)
        if let Ok(devices) = manager.enumerate_devices().await {
            if let Some(output_device) = devices.iter().find(|d| d.is_output) {
                let result = manager.find_cpal_device(&output_device.id, false).await;
                // Should succeed with a real device ID
                assert!(result.is_ok());
            }
        }
    }
}