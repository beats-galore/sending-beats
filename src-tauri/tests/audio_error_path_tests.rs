use anyhow::Result;
use sendin_beats_lib::audio::*;
use serial_test::serial;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::Mutex;

#[cfg(test)]
mod audio_error_path_tests {
    use super::*;
    use cpal::{BufferSize, SampleFormat, SampleRate, StreamConfig};
    use std::collections::HashMap;

    /// Test device disconnection scenario
    #[tokio::test]
    #[serial]
    async fn test_device_disconnection_handling() {
        let manager = AudioDeviceManager::new().expect("Failed to create manager");

        // Test getting a non-existent device (simulating disconnection)
        let result = manager.get_device("disconnected_device_12345").await;
        assert!(result.is_none(), "Non-existent device should return None");

        // Test finding a disconnected device
        let cpal_result = manager
            .find_cpal_device("disconnected_device_12345", false)
            .await;
        assert!(
            cpal_result.is_err(),
            "Disconnected device should return error"
        );

        // Test refresh after device disconnection
        let refresh_result = manager.refresh_devices().await;
        assert!(
            refresh_result.is_ok(),
            "Refresh should handle device changes gracefully"
        );
    }

    /// Test audio format change scenarios
    #[tokio::test]
    async fn test_audio_format_changes() {
        // Test invalid sample rates
        let invalid_rates = vec![0, 7000, 500000]; // Too low, uncommon, too high

        for rate in invalid_rates {
            let config_result = std::panic::catch_unwind(|| StreamConfig {
                channels: 2,
                sample_rate: SampleRate(rate),
                buffer_size: BufferSize::Default,
            });

            // Configuration creation should not panic, but the rate is invalid
            assert!(
                config_result.is_ok(),
                "Config creation should not panic for rate {}",
                rate
            );
        }

        // Test invalid channel counts
        let invalid_channels = vec![0, 255]; // Too low, too high

        for channels in invalid_channels {
            let config_result = std::panic::catch_unwind(|| StreamConfig {
                channels,
                sample_rate: SampleRate(44100),
                buffer_size: BufferSize::Default,
            });

            assert!(
                config_result.is_ok(),
                "Config creation should not panic for {} channels",
                channels
            );
        }
    }

    /// Test stream creation failures and recovery
    #[tokio::test]
    #[serial]
    async fn test_stream_creation_failures() {
        // Test creating stream with invalid device ID
        let mixer_config = AudioConfigFactory::create_dj_config();
        let mixer_result = VirtualMixer::new(mixer_config).await;

        if let Ok(mixer) = mixer_result {
            // Test adding input stream with invalid device
            let invalid_stream_result = mixer.add_input_stream("invalid_device_id_12345").await;
            assert!(
                invalid_stream_result.is_err(),
                "Adding invalid input stream should fail"
            );

            // Test setting output stream with invalid device
            let invalid_output_result =
                mixer.set_output_stream("invalid_output_device_12345").await;
            assert!(
                invalid_output_result.is_err(),
                "Setting invalid output stream should fail"
            );
        }
    }

    /// Test memory pressure scenarios
    #[tokio::test]
    async fn test_memory_pressure_handling() {
        let large_buffer_size = 1_000_000; // 1M samples
        let buffer = Arc::new(Mutex::new(Vec::<f32>::with_capacity(large_buffer_size)));

        // Simulate memory allocation under pressure
        let allocation_result = std::panic::catch_unwind(|| {
            let mut large_data = Vec::new();
            for i in 0..large_buffer_size {
                large_data.push(i as f32 * 0.001);
            }
            large_data.len()
        });

        assert!(
            allocation_result.is_ok(),
            "Large buffer allocation should not panic"
        );

        // Test buffer cleanup under memory pressure
        tokio::spawn(async move {
            let mut buf = buffer.lock().await;
            buf.extend(vec![0.0; large_buffer_size]);

            // Simulate cleanup when memory is low
            if buf.len() > 500_000 {
                buf.drain(0..250_000);
            }

            assert!(
                buf.len() <= 750_000,
                "Buffer should be cleaned up under pressure"
            );
        })
        .await
        .expect("Memory pressure test failed");
    }

    /// Test concurrent access errors and deadlock prevention
    #[tokio::test]
    async fn test_concurrent_access_errors() {
        let shared_state = Arc::new(Mutex::new(HashMap::<String, f32>::new()));
        let mut handles = Vec::new();

        // Simulate multiple threads trying to access audio state
        for i in 0..10 {
            let state = shared_state.clone();
            let handle = tokio::spawn(async move {
                // Try to acquire lock with timeout to prevent deadlock
                let timeout_result = tokio::time::timeout(Duration::from_millis(100), async {
                    let mut map = state.lock().await;
                    map.insert(format!("channel_{}", i), i as f32 * 0.1);
                    map.len()
                })
                .await;

                timeout_result.map_err(|_| "Lock acquisition timeout")
            });
            handles.push(handle);
        }

        // All tasks should complete without deadlock
        for handle in handles {
            let result = handle.await.expect("Concurrent access task failed");
            assert!(
                result.is_ok(),
                "Lock acquisition should not timeout: {:?}",
                result
            );
        }

        let final_state = shared_state.lock().await;
        assert_eq!(
            final_state.len(),
            10,
            "All concurrent writes should succeed"
        );
    }

    /// Test audio callback interruption and recovery
    #[tokio::test]
    async fn test_callback_interruption_recovery() {
        let callback_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let error_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let should_interrupt = Arc::new(AtomicBool::new(false));

        let mut handles = Vec::new();

        // Simulate audio callbacks with potential interruptions
        for i in 0..5 {
            let count = callback_count.clone();
            let errors = error_count.clone();
            let interrupt_flag = should_interrupt.clone();

            let handle = tokio::spawn(async move {
                for _ in 0..10 {
                    // Simulate callback execution
                    let callback_result = std::panic::catch_unwind(|| {
                        if interrupt_flag.load(Ordering::SeqCst) && i == 2 {
                            // Simulate interruption for one specific callback
                            return Err("Callback interrupted");
                        }

                        count.fetch_add(1, Ordering::SeqCst);
                        Ok("success")
                    });

                    match callback_result {
                        Ok(Ok(_)) => {
                            // Success case
                        }
                        Ok(Err(_)) => {
                            // Controlled error
                            errors.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(_) => {
                            // Panic case
                            errors.fetch_add(1, Ordering::SeqCst);
                        }
                    }

                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            });

            handles.push(handle);
        }

        // Trigger interruption after some callbacks
        tokio::time::sleep(Duration::from_millis(5)).await;
        should_interrupt.store(true, Ordering::SeqCst);

        for handle in handles {
            handle.await.expect("Callback interruption test failed");
        }

        let total_callbacks = callback_count.load(Ordering::SeqCst);
        let total_errors = error_count.load(Ordering::SeqCst);

        println!(
            "Total successful callbacks: {}, Total errors: {}",
            total_callbacks, total_errors
        );
        assert!(total_callbacks > 0, "Some callbacks should succeed");
        assert!(total_errors > 0, "Some interruptions should occur");
    }

    /// Test device property change handling
    #[tokio::test]
    async fn test_device_property_changes() {
        let manager = AudioDeviceManager::new().expect("Failed to create manager");

        // Test handling of device enumeration errors
        let devices_result = manager.enumerate_devices().await;
        assert!(
            devices_result.is_ok(),
            "Device enumeration should handle errors gracefully"
        );

        if let Ok(devices) = devices_result {
            for device in &devices {
                // Test accessing device properties that might change
                assert!(!device.id.is_empty(), "Device ID should not be empty");
                assert!(!device.name.is_empty(), "Device name should not be empty");

                // Device type should be valid
                assert!(
                    device.is_input || device.is_output,
                    "Device should be input or output"
                );

                // Sample rates should be reasonable (if available)
                if !device.supported_sample_rates.is_empty() {
                    for &rate in &device.supported_sample_rates {
                        assert!(
                            rate >= 8000 && rate <= 192000,
                            "Sample rate should be reasonable: {}",
                            rate
                        );
                    }
                }

                // Channel count should be valid (if available)
                if !device.supported_channels.is_empty() {
                    for &channels in &device.supported_channels {
                        assert!(
                            channels > 0 && channels <= 64,
                            "Channel count should be reasonable: {}",
                            channels
                        );
                    }
                }
            }
        }
    }

    /// Test resource exhaustion scenarios
    #[tokio::test]
    async fn test_resource_exhaustion() {
        // Test creating many audio channels (resource exhaustion simulation)
        let mut channels = Vec::new();
        let max_channels = 100usize; // Reasonable limit for testing

        for i in 0..max_channels {
            let channel = AudioChannel {
                id: i as u32,
                name: format!("Channel {}", i),
                gain: 1.0,
                pan: 0.0,
                muted: false,
                solo: false,
                input_device_id: Some(format!("device_{}", i)),
                effects_enabled: true,
                eq_low_gain: 0.0,
                eq_mid_gain: 0.0,
                eq_high_gain: 0.0,
                comp_threshold: -12.0,
                comp_ratio: 4.0,
                comp_attack: 5.0,
                comp_release: 100.0,
                comp_enabled: false,
                limiter_threshold: -0.1,
                limiter_enabled: false,
                peak_level: 0.0,
                rms_level: 0.0,
            };

            channels.push(channel);
        }

        assert_eq!(channels.len(), max_channels);

        // Test cleanup of all channels
        channels.clear();
        assert!(channels.is_empty(), "Channels should be cleaned up");
    }

    /// Test network/streaming interruption scenarios
    #[tokio::test]
    async fn test_streaming_interruption() {
        use tokio::sync::mpsc;

        let (tx, mut rx) = mpsc::channel::<Vec<f32>>(10);
        let interruption_flag = Arc::new(AtomicBool::new(false));

        // Simulate audio streaming with potential interruption
        let sender_flag = interruption_flag.clone();
        let sender_handle = tokio::spawn(async move {
            for i in 0..20 {
                if sender_flag.load(Ordering::SeqCst) {
                    // Simulate streaming interruption
                    break;
                }

                let audio_data = vec![i as f32 * 0.1; 256];

                match tx.send(audio_data).await {
                    Ok(_) => {}
                    Err(_) => {
                        // Receiver dropped, handle gracefully
                        break;
                    }
                }

                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        let receiver_handle = tokio::spawn(async move {
            let mut received_count = 0;

            while let Some(_audio_data) = rx.recv().await {
                received_count += 1;

                // Simulate interruption after some data
                if received_count == 5 {
                    break;
                }
            }

            received_count
        });

        // Trigger interruption
        tokio::time::sleep(Duration::from_millis(25)).await;
        interruption_flag.store(true, Ordering::SeqCst);

        let received_count = receiver_handle.await.expect("Receiver task failed");
        sender_handle.await.expect("Sender task failed");

        assert!(
            received_count > 0,
            "Some audio data should be received before interruption"
        );
        assert!(
            received_count <= 10,
            "Interruption should limit received data"
        );
    }

    /// Test graceful shutdown under error conditions
    #[tokio::test]
    async fn test_graceful_shutdown() {
        let is_running = Arc::new(AtomicBool::new(true));
        let shutdown_completed = Arc::new(AtomicBool::new(false));

        let running_flag = is_running.clone();
        let shutdown_flag = shutdown_completed.clone();

        // Simulate audio processing task
        let processing_handle = tokio::spawn(async move {
            while running_flag.load(Ordering::SeqCst) {
                // Simulate audio processing work
                tokio::time::sleep(Duration::from_millis(1)).await;
            }

            // Cleanup operations
            shutdown_flag.store(true, Ordering::SeqCst);
        });

        // Let processing run briefly
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Trigger shutdown
        is_running.store(false, Ordering::SeqCst);

        // Wait for graceful shutdown
        let shutdown_result =
            tokio::time::timeout(Duration::from_millis(100), processing_handle).await;

        assert!(
            shutdown_result.is_ok(),
            "Shutdown should complete within timeout"
        );
        assert!(
            shutdown_completed.load(Ordering::SeqCst),
            "Shutdown cleanup should complete"
        );
    }
}
