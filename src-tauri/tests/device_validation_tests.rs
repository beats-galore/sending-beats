use sendin_beats_lib::audio::{VirtualMixer, MixerConfig};
use tokio_test;

/// Test device ID validation and input sanitization
#[cfg(test)]
mod device_validation_tests {
    use super::*;

    async fn create_test_mixer() -> VirtualMixer {
        let config = MixerConfig::default();
        VirtualMixer::new(config).await.expect("Failed to create test mixer")
    }

    #[tokio::test]
    async fn test_valid_device_id() {
        let mixer = create_test_mixer().await;
        
        // This should fail because the device doesn't exist, but not due to validation
        let result = mixer.add_input_stream("input_test_device_123").await;
        assert!(result.is_err());
        // Should not be a validation error - should be a "device not found" error
        let error_msg = result.unwrap_err().to_string();
        assert!(!error_msg.contains("Device ID cannot be empty"));
        assert!(!error_msg.contains("invalid characters"));
    }

    #[tokio::test]
    async fn test_empty_device_id() {
        let mixer = create_test_mixer().await;
        
        let result = mixer.add_input_stream("").await;
        assert!(result.is_err(), "Empty device ID should fail validation");
        assert!(result.unwrap_err().to_string().contains("Device ID cannot be empty"));
    }

    #[tokio::test]
    async fn test_device_id_too_long() {
        let mixer = create_test_mixer().await;
        
        // Create a device ID longer than 256 characters
        let long_device_id = "a".repeat(300);
        let result = mixer.add_input_stream(&long_device_id).await;
        assert!(result.is_err(), "Long device ID should fail validation");
        assert!(result.unwrap_err().to_string().contains("Device ID too long"));
    }

    #[tokio::test]
    async fn test_device_id_invalid_characters() {
        let mixer = create_test_mixer().await;
        
        // Test various invalid characters
        let invalid_ids = vec![
            "device@id",      // @ symbol
            "device id",      // space
            "device#id",      // # symbol
            "device$id",      // $ symbol
            "device%id",      // % symbol
            "device&id",      // & symbol
            "device*id",      // * symbol
            "device(id)",     // parentheses
            "device[id]",     // brackets
            "device{id}",     // braces
            "device|id",      // pipe
            "device\\id",     // backslash
            "device/id",      // forward slash
            "device:id",      // colon
            "device;id",      // semicolon
            "device<id>",     // angle brackets
            "device=id",      // equals
            "device+id",      // plus
            "device?id",      // question mark
            "device!id",      // exclamation
        ];

        for invalid_id in invalid_ids {
            let result = mixer.add_input_stream(invalid_id).await;
            assert!(result.is_err(), "Invalid device ID '{}' should fail validation", invalid_id);
            assert!(
                result.unwrap_err().to_string().contains("invalid characters"),
                "Invalid device ID '{}' should show character validation error", invalid_id
            );
        }
    }

    #[tokio::test]
    async fn test_device_id_valid_characters() {
        let mixer = create_test_mixer().await;
        
        // Test valid characters (should not fail due to validation, but will fail due to device not found)
        let valid_ids = vec![
            "device_id",
            "device-id", 
            "device123",
            "123device",
            "Device_ID_123",
            "test-device-456",
            "input_nonexistent_test_device",  // Changed to avoid matching real devices
            "output_nonexistent_test_speakers", // Changed to avoid matching real devices
            "a",
            "1",
            "_",
            "-",
            "a1b2c3",
            "TEST_DEVICE_ID_WITH_NUMBERS_123_AND_DASHES-AND-UNDERSCORES_456",
        ];

        for valid_id in valid_ids {
            let result = mixer.add_input_stream(valid_id).await;
            // These should fail because the devices don't exist, not because of validation
            assert!(result.is_err());
            let error_msg = result.unwrap_err().to_string();
            assert!(
                !error_msg.contains("invalid characters") && !error_msg.contains("Device ID cannot be empty"),
                "Valid device ID '{}' should not fail validation: {}", valid_id, error_msg
            );
        }
    }

    #[tokio::test]
    async fn test_output_device_id_validation() {
        let mixer = create_test_mixer().await;
        
        // Test empty device ID for output stream
        let result = mixer.set_output_stream("").await;
        assert!(result.is_err(), "Empty output device ID should fail validation");
        assert!(result.unwrap_err().to_string().contains("Invalid device ID"));
    }

    #[tokio::test]
    async fn test_output_device_id_too_long() {
        let mixer = create_test_mixer().await;
        
        let long_device_id = "a".repeat(300);
        let result = mixer.set_output_stream(&long_device_id).await;
        assert!(result.is_err(), "Long output device ID should fail validation");
        assert!(result.unwrap_err().to_string().contains("Invalid device ID"));
    }

    #[tokio::test]
    async fn test_remove_input_stream_validation() {
        let mixer = create_test_mixer().await;
        
        // Remove should work even for non-existent devices (it's not an error to remove something that doesn't exist)
        let result = mixer.remove_input_stream("nonexistent_device").await;
        assert!(result.is_ok(), "Removing non-existent stream should succeed");
    }

    #[tokio::test]
    async fn test_device_id_length_boundary_conditions() {
        let mixer = create_test_mixer().await;
        
        // Test exactly 256 characters (should be valid)
        let device_id_256 = "a".repeat(256);
        let result = mixer.add_input_stream(&device_id_256).await;
        assert!(result.is_err()); // Should fail because device doesn't exist, not validation
        let error_msg = result.unwrap_err().to_string();
        assert!(!error_msg.contains("Device ID too long"), "256-character device ID should pass length validation");
        
        // Test exactly 257 characters (should be invalid)
        let device_id_257 = "a".repeat(257);
        let result = mixer.add_input_stream(&device_id_257).await;
        assert!(result.is_err(), "257-character device ID should fail validation");
        assert!(result.unwrap_err().to_string().contains("Device ID too long"));
    }

    #[tokio::test]
    async fn test_device_id_edge_cases() {
        let mixer = create_test_mixer().await;
        
        // Test single character IDs
        let result = mixer.add_input_stream("a").await;
        assert!(result.is_err()); // Device doesn't exist
        let error_msg = result.unwrap_err().to_string();
        assert!(!error_msg.contains("invalid characters"), "Single character should be valid");
        
        // Test underscore and dash combinations
        let result = mixer.add_input_stream("_-_-_").await;
        assert!(result.is_err()); // Device doesn't exist
        let error_msg = result.unwrap_err().to_string();
        assert!(!error_msg.contains("invalid characters"), "Underscore and dash combination should be valid");
    }
}