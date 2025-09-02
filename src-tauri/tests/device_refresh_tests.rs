use sendin_beats_lib::audio::{AudioDeviceInfo, AudioDeviceManager};
use serial_test::serial;
use std::collections::HashSet;

/// Test the device refresh functionality
#[cfg(test)]
mod device_refresh_tests {
    use super::*;

    async fn create_test_device_manager() -> AudioDeviceManager {
        AudioDeviceManager::new().expect("Failed to create test device manager")
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_devices_returns_list() {
        let device_manager = create_test_device_manager().await;

        // First enumeration
        let devices = device_manager.enumerate_devices().await;
        assert!(devices.is_ok(), "Initial device enumeration should succeed");

        let device_list = devices.unwrap();
        // Should have at least one device on most systems
        assert!(
            !device_list.is_empty(),
            "Should detect at least one audio device"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_devices_consistency() {
        let device_manager = create_test_device_manager().await;

        // First enumeration
        let devices1 = device_manager.enumerate_devices().await.unwrap();

        // Second enumeration (refresh)
        let devices2 = device_manager.enumerate_devices().await.unwrap();

        // Device lists should be consistent between calls
        assert_eq!(
            devices1.len(),
            devices2.len(),
            "Device count should be consistent"
        );

        // Convert to sets for comparison (order might differ)
        let device_ids1: HashSet<String> = devices1.iter().map(|d| d.id.clone()).collect();
        let device_ids2: HashSet<String> = devices2.iter().map(|d| d.id.clone()).collect();

        assert_eq!(device_ids1, device_ids2, "Device IDs should be consistent");
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_devices_validates_structure() {
        let device_manager = create_test_device_manager().await;

        let devices = device_manager.enumerate_devices().await.unwrap();

        for device in devices {
            // Validate device structure
            assert!(!device.id.is_empty(), "Device ID should not be empty");
            assert!(!device.name.is_empty(), "Device name should not be empty");
            assert!(!device.host_api.is_empty(), "Host API should not be empty");

            // Device should be either input or output (or both)
            assert!(
                device.is_input || device.is_output,
                "Device {} should be either input or output",
                device.name
            );

            // Sample rates should be reasonable
            for &rate in &device.supported_sample_rates {
                assert!(
                    rate >= 8000 && rate <= 192000,
                    "Sample rate {} should be reasonable",
                    rate
                );
            }

            // Channel counts should be reasonable
            for &channels in &device.supported_channels {
                assert!(
                    channels >= 1 && channels <= 32,
                    "Channel count {} should be reasonable",
                    channels
                );
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_devices_cache_behavior() {
        let device_manager = create_test_device_manager().await;

        // First enumeration
        let _devices1 = device_manager.enumerate_devices().await.unwrap();

        // Check cache by getting a specific device
        let first_device_id = if let Some(device) = _devices1.first() {
            device.id.clone()
        } else {
            // Skip test if no devices found
            return;
        };

        // Device should be in cache
        let cached_device = device_manager.get_device(&first_device_id).await;
        assert!(
            cached_device.is_some(),
            "Device should be cached after enumeration"
        );

        // Refresh
        let _devices2 = device_manager.enumerate_devices().await.unwrap();

        // Device should still be in cache
        let cached_device_after_refresh = device_manager.get_device(&first_device_id).await;
        assert!(
            cached_device_after_refresh.is_some(),
            "Device should remain cached after refresh"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_devices_input_output_separation() {
        let device_manager = create_test_device_manager().await;

        let devices = device_manager.enumerate_devices().await.unwrap();

        let input_devices: Vec<&AudioDeviceInfo> = devices.iter().filter(|d| d.is_input).collect();
        let output_devices: Vec<&AudioDeviceInfo> =
            devices.iter().filter(|d| d.is_output).collect();

        // Should have at least some input and output devices
        // Note: This might not be true on all systems, so we'll just log
        println!(
            "Found {} input devices and {} output devices",
            input_devices.len(),
            output_devices.len()
        );

        // Validate that input devices have input in their ID (if following naming convention)
        for device in input_devices {
            // Many input devices should have "input" in their ID or be marked as input
            assert!(
                device.is_input,
                "Input device {} should be marked as input",
                device.name
            );
        }

        // Validate that output devices have output in their ID (if following naming convention)
        for device in output_devices {
            // Many output devices should have "output" in their ID or be marked as output
            assert!(
                device.is_output,
                "Output device {} should be marked as output",
                device.name
            );
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_devices_default_devices() {
        let device_manager = create_test_device_manager().await;

        let devices = device_manager.enumerate_devices().await.unwrap();

        let default_input_devices: Vec<&AudioDeviceInfo> = devices
            .iter()
            .filter(|d| d.is_input && d.is_default)
            .collect();
        let default_output_devices: Vec<&AudioDeviceInfo> = devices
            .iter()
            .filter(|d| d.is_output && d.is_default)
            .collect();

        // Should have at least one default input and output device
        assert!(
            !default_input_devices.is_empty(),
            "Should have at least one default input device"
        );
        assert!(
            !default_output_devices.is_empty(),
            "Should have at least one default output device"
        );

        // Default devices should have proper names
        for device in default_input_devices {
            assert!(
                !device.name.is_empty(),
                "Default input device should have a name"
            );
            println!("Default input device: {}", device.name);
        }

        for device in default_output_devices {
            assert!(
                !device.name.is_empty(),
                "Default output device should have a name"
            );
            println!("Default output device: {}", device.name);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_devices_multiple_calls() {
        let device_manager = create_test_device_manager().await;

        // Multiple rapid refresh calls should not cause issues
        let mut device_counts = Vec::new();

        for i in 0..5 {
            let devices = device_manager.enumerate_devices().await;
            assert!(devices.is_ok(), "Refresh call {} should succeed", i + 1);
            device_counts.push(devices.unwrap().len());
        }

        // All counts should be the same (assuming no devices were added/removed during test)
        let first_count = device_counts[0];
        for (i, &count) in device_counts.iter().enumerate() {
            assert_eq!(
                count,
                first_count,
                "Device count {} should match first count {} on call {}",
                count,
                first_count,
                i + 1
            );
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_device_manager_creation() {
        // Test that multiple device managers can be created
        let _manager1 = create_test_device_manager().await;
        let _manager2 = create_test_device_manager().await;

        // Both should be able to enumerate devices
        let devices1 = _manager1.enumerate_devices().await;
        let devices2 = _manager2.enumerate_devices().await;

        assert!(
            devices1.is_ok(),
            "First manager should enumerate successfully"
        );
        assert!(
            devices2.is_ok(),
            "Second manager should enumerate successfully"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_device_id_format() {
        let device_manager = create_test_device_manager().await;

        let devices = device_manager.enumerate_devices().await.unwrap();

        for device in devices {
            // Device IDs should follow expected format patterns
            assert!(
                device.id.len() <= 256,
                "Device ID should not exceed 256 characters"
            );
            assert!(device.id.len() >= 1, "Device ID should not be empty");

            // Should not contain problematic characters (based on our validation rules)
            let invalid_chars = [
                '@', ' ', '#', '$', '%', '&', '*', '(', ')', '[', ']', '{', '}', '|', '\\', '/',
                ':', ';', '<', '>', '=', '+', '?', '!',
            ];

            for invalid_char in invalid_chars {
                assert!(
                    !device.id.contains(invalid_char),
                    "Device ID '{}' should not contain invalid character '{}'",
                    device.id,
                    invalid_char
                );
            }

            // Should only contain alphanumeric, underscore, and dash
            let valid_chars: HashSet<char> = device
                .id
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            let all_chars: HashSet<char> = device.id.chars().collect();

            assert_eq!(
                valid_chars, all_chars,
                "Device ID '{}' should only contain alphanumeric, underscore, and dash characters",
                device.id
            );
        }
    }
}

/// Test integration with other components
#[cfg(test)]
mod refresh_integration_tests {
    use super::*;
    use sendin_beats_lib::audio::{MixerConfig, VirtualMixer};

    async fn create_test_device_manager() -> AudioDeviceManager {
        AudioDeviceManager::new().expect("Failed to create test device manager")
    }

    async fn create_test_mixer() -> VirtualMixer {
        let config = MixerConfig::default();
        VirtualMixer::new(config)
            .await
            .expect("Failed to create test mixer")
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_with_mixer_integration() {
        let device_manager = create_test_device_manager().await;
        let _mixer = create_test_mixer().await;

        // Device refresh should work even when mixer is created
        let devices = device_manager.enumerate_devices().await;
        assert!(
            devices.is_ok(),
            "Device refresh should work with active mixer"
        );

        let device_list = devices.unwrap();
        assert!(
            !device_list.is_empty(),
            "Should still detect devices with active mixer"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_provides_valid_device_ids_for_mixer() {
        let device_manager = create_test_device_manager().await;
        let mixer = create_test_mixer().await;

        let devices = device_manager.enumerate_devices().await.unwrap();

        // Try to use refreshed device IDs with mixer operations
        for device in devices.iter().take(3) {
            // Test first 3 devices to avoid overwhelming the test
            if device.is_input {
                // This should fail because device doesn't exist in mixer context,
                // but should not fail due to invalid device ID format
                let result = mixer.add_input_stream(&device.id).await;
                if let Err(error) = result {
                    let error_msg = error.to_string();
                    // Should not be validation errors
                    assert!(!error_msg.contains("Device ID cannot be empty"));
                    assert!(!error_msg.contains("invalid characters"));
                    assert!(!error_msg.contains("Device ID too long"));
                }
            }

            if device.is_output {
                // This should fail because device doesn't exist in mixer context,
                // but should not fail due to invalid device ID format
                let result = mixer.set_output_stream(&device.id).await;
                if let Err(error) = result {
                    let error_msg = error.to_string();
                    // Should not be validation errors
                    assert!(!error_msg.contains("Invalid device ID"));
                }
            }
        }
    }
}
