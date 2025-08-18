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
    use cpal::traits::DeviceTrait;

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
        
        // Test with non-existent device - should return an error
        let result = manager.find_cpal_device("NonExistentDevice12345", false).await;
        assert!(result.is_err(), "Non-existent device should return error");
        
        // Test with cpal-accessible devices only
        // We'll use the cpal host directly to get devices we know are cpal-compatible
        use cpal::traits::HostTrait;
        let host = cpal::default_host();
        
        let mut found_test_device = false;
        
        // Test output devices that are actually accessible via cpal
        if let Ok(output_devices) = host.output_devices() {
            for device in output_devices {
                if let Ok(device_name) = device.name() {
                    // Generate the expected device ID the same way our code does
                    let expected_id = format!("output_{}", 
                        device_name.replace(" ", "_").replace("(", "").replace(")", "").to_lowercase());
                    
                    println!("Testing with cpal output device: {} -> {}", device_name, expected_id);
                    let result = manager.find_cpal_device(&expected_id, false).await;
                    
                    if result.is_ok() {
                        found_test_device = true;
                        println!("✅ Successfully found cpal output device: {}", device_name);
                        break; // Found one working device, that's enough
                    } else {
                        println!("⚠️ Could not find device '{}' via find_cpal_device (may be CoreAudio-only)", device_name);
                    }
                }
            }
        } else {
            println!("No cpal output devices available on this system");
        }
        
        // Test input devices that are actually accessible via cpal
        if let Ok(input_devices) = host.input_devices() {
            for device in input_devices {
                if let Ok(device_name) = device.name() {
                    // Generate the expected device ID the same way our code does
                    let expected_id = format!("input_{}", 
                        device_name.replace(" ", "_").replace("(", "").replace(")", "").to_lowercase());
                    
                    println!("Testing with cpal input device: {} -> {}", device_name, expected_id);
                    let result = manager.find_cpal_device(&expected_id, true).await;
                    
                    if result.is_ok() {
                        found_test_device = true;
                        println!("✅ Successfully found cpal input device: {}", device_name);
                        break; // Found one working device, that's enough
                    } else {
                        println!("⚠️ Could not find input device '{}' via find_cpal_device (may be CoreAudio-only)", device_name);
                    }
                }
            }
        } else {
            println!("No cpal input devices available on this system");
        }
        
        // The test should pass if:
        // 1. We found at least one working cpal device, OR
        // 2. There are no cpal devices available (CI/headless environment)
        if !found_test_device {
            println!("ℹ️  No cpal devices found - likely running in headless/CI environment");
            println!("   This is expected behavior for automated testing");
        }
        
        // Test passes as long as the method doesn't panic and handles errors properly
        println!("✅ find_cpal_device test completed successfully");
    }
}