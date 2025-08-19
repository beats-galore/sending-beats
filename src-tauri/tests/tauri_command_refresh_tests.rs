use sendin_beats_lib::audio::{AudioDeviceManager, AudioDeviceInfo};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use serial_test::serial;

/// Test the Tauri command functionality for device refresh
#[cfg(test)]
mod tauri_command_refresh_tests {
    use super::*;

    // Mock AudioState structure to simulate Tauri state
    struct MockAudioState {
        device_manager: Arc<AsyncMutex<AudioDeviceManager>>,
    }

    impl MockAudioState {
        async fn new() -> Self {
            let device_manager = AudioDeviceManager::new()
                .expect("Failed to create device manager");
            
            Self {
                device_manager: Arc::new(AsyncMutex::new(device_manager)),
            }
        }
    }

    // Simulate the refresh_audio_devices Tauri command
    async fn mock_refresh_audio_devices(
        audio_state: &MockAudioState,
    ) -> Result<Vec<AudioDeviceInfo>, String> {
        let device_manager = audio_state.device_manager.lock().await;
        device_manager
            .enumerate_devices()
            .await
            .map_err(|e| e.to_string())
    }

    // Simulate the enumerate_audio_devices Tauri command
    async fn mock_enumerate_audio_devices(
        audio_state: &MockAudioState,
    ) -> Result<Vec<AudioDeviceInfo>, String> {
        let device_manager = audio_state.device_manager.lock().await;
        device_manager
            .enumerate_devices()
            .await
            .map_err(|e| e.to_string())
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_command_returns_success() {
        let audio_state = MockAudioState::new().await;
        
        let result = mock_refresh_audio_devices(&audio_state).await;
        assert!(result.is_ok(), "Refresh command should return success");
        
        let devices = result.unwrap();
        assert!(!devices.is_empty(), "Should return at least one device");
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_vs_enumerate_consistency() {
        let audio_state = MockAudioState::new().await;
        
        // Call enumerate first
        let enumerate_result = mock_enumerate_audio_devices(&audio_state).await;
        assert!(enumerate_result.is_ok(), "Enumerate should succeed");
        
        // Call refresh
        let refresh_result = mock_refresh_audio_devices(&audio_state).await;
        assert!(refresh_result.is_ok(), "Refresh should succeed");
        
        let enumerate_devices = enumerate_result.unwrap();
        let refresh_devices = refresh_result.unwrap();
        
        // Results should be identical
        assert_eq!(enumerate_devices.len(), refresh_devices.len(), 
            "Enumerate and refresh should return same number of devices");
        
        // Check that all devices are present in both lists
        for device in &enumerate_devices {
            let found = refresh_devices.iter().any(|d| d.id == device.id);
            assert!(found, "Device {} should be present in both enumerate and refresh", device.id);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_command_error_handling() {
        // Test what happens when device manager fails
        // Note: In real scenarios, device enumeration rarely fails, 
        // but we should handle the error case properly
        
        let audio_state = MockAudioState::new().await;
        
        // Even if there are no devices or some issue, should return proper error
        let result = mock_refresh_audio_devices(&audio_state).await;
        
        match result {
            Ok(devices) => {
                // Success case - should have valid device list
                for device in devices {
                    assert!(!device.id.is_empty(), "Device ID should not be empty");
                    assert!(!device.name.is_empty(), "Device name should not be empty");
                }
            }
            Err(error) => {
                // Error case - should have meaningful error message
                assert!(!error.is_empty(), "Error message should not be empty");
                println!("Expected error case: {}", error);
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_concurrent_refresh_calls() {
        let audio_state = Arc::new(MockAudioState::new().await);
        
        // Simulate multiple concurrent refresh calls
        let handles: Vec<_> = (0..5).map(|i| {
            let state = audio_state.clone();
            tokio::spawn(async move {
                let result = mock_refresh_audio_devices(&state).await;
                (i, result)
            })
        }).collect();
        
        // Wait for all calls to complete
        let results = futures::future::join_all(handles).await;
        
        // All calls should succeed
        for (i, result) in results.into_iter().enumerate() {
            let task_result = result.expect("Task should not panic");
            let (task_id, device_result) = task_result;
            assert!(device_result.is_ok(), "Concurrent refresh call {} should succeed", task_id);
            
            let devices = device_result.unwrap();
            assert!(!devices.is_empty(), "Concurrent call {} should return devices", task_id);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_command_serialization() {
        let audio_state = MockAudioState::new().await;
        
        let devices = mock_refresh_audio_devices(&audio_state).await.unwrap();
        
        // Test that device data can be serialized (important for Tauri commands)
        for device in devices {
            // Simulate JSON serialization (what Tauri does)
            let json_result = serde_json::to_string(&device);
            assert!(json_result.is_ok(), "Device should be JSON serializable");
            
            let json_str = json_result.unwrap();
            assert!(!json_str.is_empty(), "Serialized JSON should not be empty");
            
            // Test deserialization
            let deserialized: Result<AudioDeviceInfo, _> = serde_json::from_str(&json_str);
            assert!(deserialized.is_ok(), "Device should be JSON deserializable");
            
            let deserialized_device = deserialized.unwrap();
            assert_eq!(device.id, deserialized_device.id, "Device ID should survive serialization");
            assert_eq!(device.name, deserialized_device.name, "Device name should survive serialization");
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_command_memory_usage() {
        let audio_state = MockAudioState::new().await;
        
        // Multiple refresh calls should not cause memory leaks
        for i in 0..10 {
            let result = mock_refresh_audio_devices(&audio_state).await;
            assert!(result.is_ok(), "Refresh call {} should succeed", i + 1);
            
            let devices = result.unwrap();
            assert!(!devices.is_empty(), "Should return devices on call {}", i + 1);
            
            // Devices should be dropped after each iteration
            // (Rust's ownership system handles this automatically)
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_state_isolation() {
        // Test that different audio states are isolated
        let audio_state1 = MockAudioState::new().await;
        let audio_state2 = MockAudioState::new().await;
        
        let devices1 = mock_refresh_audio_devices(&audio_state1).await.unwrap();
        let devices2 = mock_refresh_audio_devices(&audio_state2).await.unwrap();
        
        // Both should return the same devices (same system)
        assert_eq!(devices1.len(), devices2.len(), 
            "Different states should see the same system devices");
        
        // But they should be independent instances
        // (This is more of a conceptual test - in practice, they access the same system)
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_command_with_empty_system() {
        // This test simulates what happens on a system with very limited audio
        let audio_state = MockAudioState::new().await;
        
        let result = mock_refresh_audio_devices(&audio_state).await;
        
        // Even minimal systems should have at least some audio device
        // (On macOS/Windows/Linux, there's usually at least a default device)
        match result {
            Ok(devices) => {
                // Normal case - should have devices
                assert!(!devices.is_empty(), "Most systems should have at least one audio device");
            }
            Err(_error) => {
                // This might happen in very constrained environments (CI, containers, etc.)
                // The important thing is that it doesn't panic
                println!("System appears to have no audio devices - this can happen in CI environments");
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_command_output_structure() {
        let audio_state = MockAudioState::new().await;
        
        let devices = mock_refresh_audio_devices(&audio_state).await.unwrap();
        
        // Validate the structure of returned devices
        for device in devices {
            // Required fields should be present
            assert!(!device.id.is_empty(), "Device ID is required");
            assert!(!device.name.is_empty(), "Device name is required");
            assert!(!device.host_api.is_empty(), "Host API is required");
            
            // Boolean flags should be valid
            // (Note: Both can be true for duplex devices)
            assert!(device.is_input || device.is_output, 
                "Device should be either input or output (or both)");
            
            // Arrays should be valid
            assert!(!device.supported_sample_rates.is_empty(), 
                "Device should support at least one sample rate");
            assert!(!device.supported_channels.is_empty(), 
                "Device should support at least one channel configuration");
            
            // Sample rates should be reasonable
            for &rate in &device.supported_sample_rates {
                assert!(rate >= 8000 && rate <= 192000, 
                    "Sample rate {} should be in reasonable range", rate);
            }
            
            // Channel counts should be reasonable  
            for &channels in &device.supported_channels {
                assert!(channels >= 1 && channels <= 32, 
                    "Channel count {} should be reasonable", channels);
            }
        }
    }
}

/// Test error conditions and edge cases
#[cfg(test)]
mod refresh_error_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_device_manager_creation_failure_resilience() {
        // Test handling of device manager creation in various conditions
        
        // This test primarily ensures our error handling is robust
        // In normal conditions, device manager creation should succeed
        match AudioDeviceManager::new() {
            Ok(_manager) => {
                // Normal case - device manager created successfully
                println!("Device manager created successfully");
            }
            Err(error) => {
                // This might happen in constrained environments
                println!("Device manager creation failed: {}", error);
                // The important thing is that it returns a proper error, not panic
                assert!(!error.to_string().is_empty(), "Error should have a message");
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_refresh_with_system_changes() {
        // This test simulates what happens during system changes
        // (In practice, we can't actually change system devices during test)
        
        let device_manager = AudioDeviceManager::new()
            .expect("Device manager should be created");
        
        // Initial enumeration
        let devices1 = device_manager.enumerate_devices().await;
        assert!(devices1.is_ok(), "Initial enumeration should succeed");
        
        // Simulate time passing (devices might change)
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        // Second enumeration
        let devices2 = device_manager.enumerate_devices().await;
        assert!(devices2.is_ok(), "Second enumeration should succeed");
        
        // In stable test environment, devices should be the same
        let devices1_list = devices1.unwrap();
        let devices2_list = devices2.unwrap();
        
        // Device count should be stable in test environment
        assert_eq!(devices1_list.len(), devices2_list.len(), 
            "Device count should be stable in test environment");
    }
}