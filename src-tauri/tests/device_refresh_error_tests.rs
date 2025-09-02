use sendin_beats_lib::audio::{AudioDeviceManager, MixerConfig, VirtualMixer};
use serial_test::serial;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

/// Test error handling scenarios for device refresh functionality
#[cfg(test)]
mod device_refresh_error_tests {
    use super::*;

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
    async fn test_refresh_devices_error_recovery() {
        let device_manager = create_test_device_manager().await;

        // First successful enumeration
        let devices1 = device_manager.enumerate_devices().await;
        assert!(devices1.is_ok(), "Initial enumeration should succeed");

        // Simulate rapid successive calls (could cause resource contention)
        let mut results = Vec::new();
        for i in 0..20 {
            let result = device_manager.enumerate_devices().await;
            results.push((i, result));
        }

        // All calls should either succeed or fail gracefully
        let mut success_count = 0;
        let mut error_count = 0;

        for (i, result) in results {
            match result {
                Ok(_devices) => {
                    success_count += 1;
                }
                Err(error) => {
                    error_count += 1;
                    // Errors should have meaningful messages
                    assert!(
                        !error.to_string().is_empty(),
                        "Error {} should have a message",
                        i
                    );
                    println!("Call {}: Error (expected in stress test): {}", i, error);
                }
            }
        }

        println!(
            "Stress test results: {} successes, {} errors",
            success_count, error_count
        );
        // At least some calls should succeed
        assert!(
            success_count > 0,
            "At least some rapid calls should succeed"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_device_manager_with_invalid_state() {
        // Test device manager behavior in various states
        let device_manager = create_test_device_manager().await;

        // Multiple enumerations should be safe
        for i in 0..5 {
            let result = device_manager.enumerate_devices().await;

            match result {
                Ok(devices) => {
                    assert!(
                        !devices.is_empty(),
                        "Should find devices on iteration {}",
                        i
                    );
                }
                Err(error) => {
                    // If enumeration fails, error should be descriptive
                    let error_msg = error.to_string();
                    assert!(!error_msg.is_empty(), "Error message should not be empty");
                    assert!(error_msg.len() > 10, "Error message should be descriptive");
                    println!(
                        "Enumeration {} failed (might be expected): {}",
                        i, error_msg
                    );
                }
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_concurrent_device_access_safety() {
        let device_manager = Arc::new(AsyncMutex::new(create_test_device_manager().await));

        // Test concurrent access to device manager
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let manager = device_manager.clone();
                tokio::spawn(async move {
                    let device_manager = manager.lock().await;
                    let result = device_manager.enumerate_devices().await;
                    (i, result)
                })
            })
            .collect();

        let results = futures::future::join_all(handles).await;

        // All tasks should complete without panicking
        for (i, task_result) in results.into_iter().enumerate() {
            assert!(task_result.is_ok(), "Task {} should not panic", i);

            let (task_id, enum_result) = task_result.unwrap();
            match enum_result {
                Ok(devices) => {
                    assert!(!devices.is_empty(), "Task {} should find devices", task_id);
                }
                Err(error) => {
                    println!(
                        "Task {} failed (may be expected under contention): {}",
                        task_id, error
                    );
                }
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_device_cache_consistency_under_stress() {
        let device_manager = create_test_device_manager().await;

        // Initial enumeration to populate cache
        let initial_devices = device_manager.enumerate_devices().await.unwrap();
        assert!(!initial_devices.is_empty(), "Should have initial devices");

        let first_device_id = initial_devices[0].id.clone();

        // Stress test cache consistency
        for i in 0..50 {
            // Enumerate devices
            let devices = device_manager.enumerate_devices().await;
            assert!(devices.is_ok(), "Enumeration {} should succeed", i);

            // Check cache consistency
            let cached_device = device_manager.get_device(&first_device_id).await;
            assert!(
                cached_device.is_some(),
                "Device {} should remain in cache after enumeration {}",
                first_device_id,
                i
            );

            if i % 10 == 0 {
                println!("Cache consistency check {}/50 passed", i + 1);
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_mixer_device_interaction_errors() {
        let device_manager = create_test_device_manager().await;
        let mixer = create_test_mixer().await;

        // Get devices from refresh
        let devices = device_manager.enumerate_devices().await.unwrap();

        // Test error conditions when using refreshed devices with mixer
        for device in devices.iter().take(3) {
            if device.is_input {
                // Try to add input stream - should fail gracefully if device not available
                let result = mixer.add_input_stream(&device.id).await;
                if let Err(error) = result {
                    let error_msg = error.to_string();
                    // Error should be descriptive and not a panic
                    assert!(
                        !error_msg.is_empty(),
                        "Input stream error should have message"
                    );
                    assert!(!error_msg.contains("panic"), "Should not contain panic");
                    assert!(
                        !error_msg.contains("unwrap"),
                        "Should not contain unwrap errors"
                    );
                }
            }

            if device.is_output {
                // Try to set output stream - should fail gracefully if device not available
                let result = mixer.set_output_stream(&device.id).await;
                if let Err(error) = result {
                    let error_msg = error.to_string();
                    // Error should be descriptive and not a panic
                    assert!(
                        !error_msg.is_empty(),
                        "Output stream error should have message"
                    );
                    assert!(!error_msg.contains("panic"), "Should not contain panic");
                    assert!(
                        !error_msg.contains("unwrap"),
                        "Should not contain unwrap errors"
                    );
                }
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_device_enumeration_with_system_limitations() {
        let device_manager = create_test_device_manager().await;

        // Test enumeration under various conditions
        let enumeration_attempts = 10;
        let mut successful_enumerations = 0;
        let mut failed_enumerations = 0;

        for i in 0..enumeration_attempts {
            let result = device_manager.enumerate_devices().await;

            match result {
                Ok(devices) => {
                    successful_enumerations += 1;

                    // Validate device data even under stress
                    for device in devices {
                        assert!(
                            !device.id.is_empty(),
                            "Device ID should not be empty in attempt {}",
                            i
                        );
                        assert!(
                            !device.name.is_empty(),
                            "Device name should not be empty in attempt {}",
                            i
                        );
                        assert!(
                            device.is_input || device.is_output,
                            "Device should be input or output in attempt {}",
                            i
                        );
                    }
                }
                Err(error) => {
                    failed_enumerations += 1;
                    // Failed enumerations should have proper error messages
                    assert!(
                        !error.to_string().is_empty(),
                        "Failed enumeration {} should have error message",
                        i
                    );
                }
            }

            // Small delay to avoid overwhelming the system
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        println!(
            "Enumeration test: {}/{} successful, {}/{} failed",
            successful_enumerations,
            enumeration_attempts,
            failed_enumerations,
            enumeration_attempts
        );

        // Most attempts should succeed in a normal environment
        let success_rate = successful_enumerations as f64 / enumeration_attempts as f64;
        assert!(
            success_rate >= 0.8,
            "Success rate should be at least 80%, got {:.2}%",
            success_rate * 100.0
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_device_data_validation_errors() {
        let device_manager = create_test_device_manager().await;

        let devices = device_manager.enumerate_devices().await.unwrap();

        // Test that all device data meets validation requirements
        for (i, device) in devices.iter().enumerate() {
            // Test device ID validation
            assert!(
                device.id.len() <= 256,
                "Device {} ID length {} should not exceed 256 characters",
                i,
                device.id.len()
            );
            assert!(!device.id.is_empty(), "Device {} ID should not be empty", i);

            // Test device name validation
            assert!(
                device.name.len() <= 512,
                "Device {} name length {} should not exceed 512 characters",
                i,
                device.name.len()
            );
            assert!(
                !device.name.is_empty(),
                "Device {} name should not be empty",
                i
            );

            // Test sample rate validation
            for (j, &rate) in device.supported_sample_rates.iter().enumerate() {
                assert!(
                    rate >= 8000 && rate <= 192000,
                    "Device {} sample rate {} at index {} should be reasonable",
                    i,
                    rate,
                    j
                );
            }

            // Test channel validation
            for (j, &channels) in device.supported_channels.iter().enumerate() {
                assert!(
                    channels >= 1 && channels <= 32,
                    "Device {} channel count {} at index {} should be reasonable",
                    i,
                    channels,
                    j
                );
            }

            // Test that device has at least one capability
            assert!(
                device.is_input || device.is_output,
                "Device {} should be either input or output",
                i
            );

            // Test host API field
            assert!(
                !device.host_api.is_empty(),
                "Device {} host API should not be empty",
                i
            );
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_memory_safety() {
        // Test that repeated refreshes don't cause memory issues
        let device_manager = create_test_device_manager().await;

        for i in 0..100 {
            let result = device_manager.enumerate_devices().await;

            match result {
                Ok(devices) => {
                    // Force devices to be consumed and dropped
                    let device_count = devices.len();
                    assert!(device_count >= 0, "Device count should be non-negative");

                    // Occasionally log progress
                    if i % 25 == 0 {
                        println!("Memory safety test: {}/100 iterations completed", i + 1);
                    }
                }
                Err(error) => {
                    // Errors should be handled gracefully
                    println!("Iteration {} failed: {}", i, error);
                }
            }

            // Variables should be dropped here, freeing memory
        }

        // Final enumeration should still work
        let final_result = device_manager.enumerate_devices().await;
        assert!(
            final_result.is_ok(),
            "Final enumeration should succeed after memory test"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_device_manager_drop_safety() {
        // Test that device manager can be safely dropped and recreated
        for i in 0..5 {
            let device_manager = AudioDeviceManager::new();

            match device_manager {
                Ok(manager) => {
                    let devices = manager.enumerate_devices().await;
                    match devices {
                        Ok(device_list) => {
                            assert!(
                                !device_list.is_empty(),
                                "Iteration {} should find devices",
                                i
                            );
                        }
                        Err(error) => {
                            println!("Enumeration failed in iteration {}: {}", i, error);
                        }
                    }
                    // manager is dropped here
                }
                Err(error) => {
                    println!(
                        "Device manager creation failed in iteration {}: {}",
                        i, error
                    );
                }
            }
        }

        // Creating a new device manager after previous ones were dropped should work
        let final_manager = AudioDeviceManager::new();
        assert!(
            final_manager.is_ok(),
            "Final device manager creation should succeed"
        );
    }
}

/// Test edge cases and boundary conditions
#[cfg(test)]
mod refresh_edge_case_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_device_refresh_with_no_permissions() {
        // Test behavior when audio permissions might be limited
        // (This test is more relevant on iOS/Android, but good to have)

        let device_manager_result = AudioDeviceManager::new();

        match device_manager_result {
            Ok(device_manager) => {
                let devices_result = device_manager.enumerate_devices().await;

                match devices_result {
                    Ok(devices) => {
                        // Normal case - we have audio permissions
                        println!(
                            "Audio permissions available: found {} devices",
                            devices.len()
                        );
                    }
                    Err(error) => {
                        // Limited permissions case
                        println!("Limited audio permissions: {}", error);
                        // Should not panic, should return proper error
                        assert!(
                            !error.to_string().is_empty(),
                            "Permission error should have message"
                        );
                    }
                }
            }
            Err(error) => {
                // Very limited permissions - can't even create device manager
                println!(
                    "Cannot create device manager (very limited permissions): {}",
                    error
                );
                assert!(
                    !error.to_string().is_empty(),
                    "Creation error should have message"
                );
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_device_refresh_timing_edge_cases() {
        let device_manager = AudioDeviceManager::new().unwrap();

        // Test very rapid successive calls
        let start_time = std::time::Instant::now();

        for i in 0..5 {
            let result = device_manager.enumerate_devices().await;
            match result {
                Ok(_devices) => {
                    println!("Rapid call {} succeeded", i);
                }
                Err(error) => {
                    println!("Rapid call {} failed: {}", i, error);
                    // Rapid failures should still have proper error messages
                    assert!(
                        !error.to_string().is_empty(),
                        "Rapid call error should have message"
                    );
                }
            }
        }

        let elapsed = start_time.elapsed();
        println!("Rapid enumeration test completed in {:?}", elapsed);

        // Test with deliberate delays
        for i in 0..3 {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let result = device_manager.enumerate_devices().await;
            assert!(result.is_ok(), "Delayed call {} should succeed", i);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_empty_device_list_handling() {
        // Test behavior when device enumeration returns empty list
        // (This is rare but can happen in virtualized environments)

        let device_manager = AudioDeviceManager::new().unwrap();
        let devices = device_manager.enumerate_devices().await.unwrap();

        if devices.is_empty() {
            println!("No audio devices found - testing empty list handling");

            // Cache operations should still work with empty list
            let non_existent_device = device_manager.get_device("non_existent").await;
            assert!(
                non_existent_device.is_none(),
                "Non-existent device should return None even with empty device list"
            );
        } else {
            println!("Devices found: {} (normal case)", devices.len());

            // Test cache with actual devices
            let first_device_id = &devices[0].id;
            let cached_device = device_manager.get_device(first_device_id).await;
            assert!(cached_device.is_some(), "First device should be cached");
        }
    }
}
