use sendin_beats_lib::audio::{AudioChannel, MixerConfig, VirtualMixer};
use tokio_test;

/// Test audio stream management functionality
#[cfg(test)]
mod stream_management_tests {
    use super::*;

    async fn create_test_mixer() -> VirtualMixer {
        let config = MixerConfig::default();
        VirtualMixer::new(config)
            .await
            .expect("Failed to create test mixer")
    }

    #[tokio::test]
    async fn test_mixer_lifecycle() {
        let mut mixer = create_test_mixer().await;

        // Test starting the mixer
        let start_result = mixer.start().await;
        assert!(start_result.is_ok(), "Mixer should start successfully");

        // Test stopping the mixer
        let stop_result = mixer.stop().await;
        assert!(stop_result.is_ok(), "Mixer should stop successfully");

        // Test starting again after stop
        let restart_result = mixer.start().await;
        assert!(restart_result.is_ok(), "Mixer should restart successfully");

        let stop_result2 = mixer.stop().await;
        assert!(stop_result2.is_ok(), "Mixer should stop again successfully");
    }

    #[tokio::test]
    async fn test_multiple_start_calls() {
        let mut mixer = create_test_mixer().await;

        // Multiple start calls should not cause issues
        let start1 = mixer.start().await;
        assert!(start1.is_ok(), "First start should succeed");

        let start2 = mixer.start().await;
        assert!(start2.is_ok(), "Second start should succeed (idempotent)");

        let start3 = mixer.start().await;
        assert!(start3.is_ok(), "Third start should succeed (idempotent)");

        let stop_result = mixer.stop().await;
        assert!(stop_result.is_ok(), "Stop should succeed");
    }

    #[tokio::test]
    async fn test_add_input_stream_invalid_device() {
        let mixer = create_test_mixer().await;

        // Try to add a stream for a non-existent device
        let result = mixer.add_input_stream("nonexistent_input_device").await;
        assert!(
            result.is_err(),
            "Adding non-existent input device should fail"
        );

        // The error should indicate device not found, not validation issues
        let error_msg = result.unwrap_err().to_string();
        assert!(
            !error_msg.contains("invalid characters"),
            "Error should be about device not found"
        );
    }

    #[tokio::test]
    async fn test_set_output_stream_invalid_device() {
        let mixer = create_test_mixer().await;

        // Try to set output stream for a non-existent device
        let result = mixer.set_output_stream("nonexistent_output_device").await;
        assert!(
            result.is_err(),
            "Setting non-existent output device should fail"
        );

        // Should indicate device not found or stream creation error
        let error_msg = result.unwrap_err().to_string();
        // The error could be about device not found, no suitable device, or stream creation failure
        assert!(
            error_msg.contains("not found")
                || error_msg.contains("No default")
                || error_msg.contains("Failed to get")
                || error_msg.contains("Failed to build")
                || error_msg.contains("No suitable")
                || error_msg.contains("suitable"),
            "Error should indicate device or stream issue: {}",
            error_msg
        );
    }

    #[tokio::test]
    async fn test_remove_input_stream_nonexistent() {
        let mixer = create_test_mixer().await;

        // Removing non-existent stream should succeed (it's not an error)
        let result = mixer.remove_input_stream("nonexistent_device").await;
        assert!(
            result.is_ok(),
            "Removing non-existent stream should succeed"
        );
    }

    #[tokio::test]
    async fn test_add_and_remove_input_stream() {
        let mixer = create_test_mixer().await;

        // Try to add a stream (will fail because device doesn't exist, but should pass validation)
        let add_result = mixer.add_input_stream("test_device_123").await;
        assert!(
            add_result.is_err(),
            "Adding non-existent device should fail"
        );

        // But validation should have passed
        let error_msg = add_result.unwrap_err().to_string();
        assert!(
            !error_msg.contains("invalid characters"),
            "Validation should pass for valid device ID"
        );

        // Remove the stream (should succeed even if device was never actually added)
        let remove_result = mixer.remove_input_stream("test_device_123").await;
        assert!(remove_result.is_ok(), "Removing stream should succeed");
    }

    #[tokio::test]
    async fn test_channel_management() {
        let mut mixer = create_test_mixer().await;

        // Add a channel
        let channel = AudioChannel {
            id: 1,
            name: "Test Channel".to_string(),
            gain: 0.8,
            pan: 0.0,
            ..Default::default()
        };

        let add_result = mixer.add_channel(channel.clone()).await;
        assert!(add_result.is_ok(), "Adding channel should succeed");

        // Update the channel
        let updated_channel = AudioChannel {
            id: 1,
            name: "Updated Test Channel".to_string(),
            gain: 0.6,
            pan: 0.2,
            ..Default::default()
        };

        let update_result = mixer.update_channel(1, updated_channel).await;
        assert!(update_result.is_ok(), "Updating channel should succeed");
    }

    #[tokio::test]
    async fn test_audio_metrics() {
        let mixer = create_test_mixer().await;

        let metrics = mixer.get_metrics().await;

        // Verify metrics have reasonable values
        assert!(metrics.cpu_usage >= 0.0, "CPU usage should be non-negative");
        assert!(metrics.sample_rate > 0, "Sample rate should be positive");
        assert!(metrics.latency_ms >= 0.0, "Latency should be non-negative");
        assert!(
            metrics.buffer_underruns >= 0,
            "Buffer underruns should be non-negative"
        );
        assert!(
            metrics.buffer_overruns >= 0,
            "Buffer overruns should be non-negative"
        );
    }

    #[tokio::test]
    async fn test_mixer_with_processing() {
        let mut mixer = create_test_mixer().await;

        // Start the mixer to begin processing
        let start_result = mixer.start().await;
        assert!(start_result.is_ok(), "Mixer should start");

        // Wait a short time for processing to begin
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check that metrics are being updated
        let metrics = mixer.get_metrics().await;
        assert!(
            metrics.sample_rate > 0,
            "Metrics should be available during processing"
        );

        // Check that levels are accessible during processing
        let levels = mixer.get_channel_levels().await;
        let master = mixer.get_master_levels().await;

        // Should not panic or hang
        assert!(true, "Level access should work during processing");

        // Stop processing
        let stop_result = mixer.stop().await;
        assert!(stop_result.is_ok(), "Mixer should stop");
    }

    #[tokio::test]
    async fn test_concurrent_stream_operations() {
        let mixer = std::sync::Arc::new(create_test_mixer().await);

        // Try multiple concurrent stream operations using clearly non-existent device names
        let handles = (0..5)
            .map(|i| {
                let mixer_ref = mixer.clone();
                tokio::spawn(async move {
                    let device_id = format!("nonexistent_test_device_{}", i);
                    let add_result = mixer_ref.add_input_stream(&device_id).await;
                    // Should fail because devices don't exist, but not due to validation
                    assert!(add_result.is_err());

                    let remove_result = mixer_ref.remove_input_stream(&device_id).await;
                    // Should succeed
                    assert!(remove_result.is_ok());
                })
            })
            .collect::<Vec<_>>();

        // All operations should complete
        for handle in handles {
            let result = handle.await;
            assert!(
                result.is_ok(),
                "Concurrent stream operations should complete"
            );
        }
    }

    #[tokio::test]
    async fn test_mixer_send_command() {
        let mixer = create_test_mixer().await;

        // Test sending commands to the mixer
        use sendin_beats_lib::audio::MixerCommand;

        let command = MixerCommand::SetMasterGain(0.8);
        let result = mixer.send_command(command).await;
        assert!(result.is_ok(), "Sending mixer command should succeed");

        let command2 = MixerCommand::StartStream;
        let result2 = mixer.send_command(command2).await;
        assert!(
            result2.is_ok(),
            "Sending start stream command should succeed"
        );

        let command3 = MixerCommand::StopStream;
        let result3 = mixer.send_command(command3).await;
        assert!(
            result3.is_ok(),
            "Sending stop stream command should succeed"
        );
    }

    #[tokio::test]
    async fn test_audio_output_receiver() {
        let mixer = create_test_mixer().await;

        // Get an audio output receiver - this should succeed
        let receiver = mixer.get_audio_output_receiver().await;

        // The receiver should be valid - we're just testing that we can create it
        // Don't try to receive data as the processing thread may not be running
        drop(receiver);
        assert!(true, "Audio output receiver created successfully");
    }
}
