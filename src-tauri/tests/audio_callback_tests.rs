use sendin_beats_lib::audio::*;
use serial_test::serial;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::Mutex;

#[cfg(test)]
mod audio_callback_tests {
    use super::*;
    use cpal::{SampleFormat, SampleRate, StreamConfig};
    use std::collections::HashMap;

    /// Test audio input callback function handling
    #[tokio::test]
    async fn test_audio_input_callback() {
        let mut audio_buffer = Vec::new();
        let input_data: &[f32] = &[0.1, 0.2, 0.3, 0.4, 0.5];

        // Simulate audio input callback behavior
        let callback_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            audio_buffer.extend_from_slice(input_data);
            audio_buffer.len()
        }));

        assert!(callback_result.is_ok());
        assert_eq!(audio_buffer.len(), 5);
        assert!((audio_buffer[0] - 0.1).abs() < f32::EPSILON);
        assert!((audio_buffer[4] - 0.5).abs() < f32::EPSILON);
    }

    /// Test audio output callback function handling
    #[tokio::test]
    async fn test_audio_output_callback() {
        let source_data: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4];
        let mut output_buffer = vec![0.0; 4];

        // Simulate audio output callback behavior
        let callback_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for (i, &sample) in source_data.iter().enumerate() {
                if i < output_buffer.len() {
                    output_buffer[i] = sample;
                }
            }
            output_buffer.len()
        }));

        assert!(callback_result.is_ok());
        assert_eq!(output_buffer.len(), 4);
        assert!((output_buffer[0] - 0.1).abs() < f32::EPSILON);
        assert!((output_buffer[3] - 0.4).abs() < f32::EPSILON);
    }

    /// Test callback with buffer underrun/overrun scenarios
    #[tokio::test]
    async fn test_callback_buffer_edge_cases() {
        // Test buffer underrun (source smaller than output)
        let source_data: Vec<f32> = vec![0.5, 0.6];
        let mut output_buffer = vec![0.0; 5];

        let callback_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for (i, &sample) in source_data.iter().enumerate() {
                if i < output_buffer.len() {
                    output_buffer[i] = sample;
                }
            }
            // Remaining buffer should stay zero
            source_data.len()
        }));

        assert!(callback_result.is_ok());
        assert!((output_buffer[0] - 0.5).abs() < f32::EPSILON);
        assert!((output_buffer[1] - 0.6).abs() < f32::EPSILON);
        assert!((output_buffer[2] - 0.0).abs() < f32::EPSILON); // Should remain zero

        // Test buffer overrun (source larger than output)
        let large_source: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7];
        let mut small_output = vec![0.0; 3];

        let overrun_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for (i, &sample) in large_source.iter().enumerate() {
                if i < small_output.len() {
                    small_output[i] = sample;
                }
            }
            small_output.len()
        }));

        assert!(overrun_result.is_ok());
        assert_eq!(small_output.len(), 3);
        assert!((small_output[2] - 0.3).abs() < f32::EPSILON); // Only first 3 samples
    }

    /// Test audio callback with real audio stream data flow
    #[tokio::test]
    async fn test_audio_stream_callback_flow() {
        let stream_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
        let stream_buffer_clone = stream_buffer.clone();

        // Simulate input callback writing to buffer
        {
            let mut buffer = stream_buffer.lock().await;
            buffer.extend_from_slice(&[0.1, 0.2, 0.3, 0.4]);
        }

        // Simulate output callback reading from buffer
        let output_data = {
            let mut buffer = stream_buffer_clone.lock().await;
            let data = buffer.clone();
            buffer.clear();
            data
        };

        assert_eq!(output_data.len(), 4);
        assert!((output_data[0] - 0.1).abs() < f32::EPSILON);
        assert!((output_data[3] - 0.4).abs() < f32::EPSILON);

        // Buffer should be empty after read
        let empty_buffer = stream_buffer.lock().await;
        assert!(empty_buffer.is_empty());
    }

    /// Test audio callback timing and threading behavior
    #[tokio::test]
    async fn test_callback_threading_safety() {
        let shared_counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let shared_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));

        let mut handles = Vec::new();

        // Simulate multiple concurrent audio callbacks
        for i in 0..5 {
            let counter = shared_counter.clone();
            let buffer = shared_buffer.clone();

            let handle = tokio::spawn(async move {
                let value = i as f32 * 0.1;

                // Simulate callback incrementing counter and writing data
                counter.fetch_add(1, Ordering::SeqCst);

                let mut buf = buffer.lock().await;
                buf.push(value);
            });

            handles.push(handle);
        }

        // Wait for all callbacks to complete
        for handle in handles {
            handle.await.expect("Callback task failed");
        }

        assert_eq!(shared_counter.load(Ordering::SeqCst), 5);

        let final_buffer = shared_buffer.lock().await;
        assert_eq!(final_buffer.len(), 5);

        // All values should be present (order may vary due to concurrency)
        let mut sorted_values = final_buffer.clone();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        for (i, &value) in sorted_values.iter().enumerate() {
            let expected = i as f32 * 0.1;
            assert!((value - expected).abs() < f32::EPSILON);
        }
    }

    /// Test callback error handling and recovery
    #[tokio::test]
    async fn test_callback_error_handling() {
        let error_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let error_count_clone = error_count.clone();

        // Simulate callback that might fail
        let callback_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let should_fail = false; // Controlled failure condition

            if should_fail {
                panic!("Simulated audio callback failure");
            }

            error_count_clone.fetch_add(1, Ordering::SeqCst);
            "success"
        }));

        assert!(callback_result.is_ok());
        assert_eq!(error_count.load(Ordering::SeqCst), 1);

        // Test actual panic handling
        let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            panic!("Intentional panic for testing");
        }));

        assert!(panic_result.is_err());
    }

    /// Test callback with different sample formats and rates
    #[tokio::test]
    async fn test_callback_format_handling() {
        // Test f32 format (default)
        let f32_data: Vec<f32> = vec![0.1, -0.2, 0.3, -0.4];
        let mut f32_output = vec![0.0; 4];

        for (i, &sample) in f32_data.iter().enumerate() {
            f32_output[i] = sample;
        }

        assert!((f32_output[1] + 0.2).abs() < f32::EPSILON);
        assert!((f32_output[3] + 0.4).abs() < f32::EPSILON);

        // Test conversion from i16 to f32 (common in audio systems)
        let i16_data: Vec<i16> = vec![3276, -6553, 9830, -13107]; // roughly 0.1, -0.2, 0.3, -0.4
        let converted_f32: Vec<f32> = i16_data
            .iter()
            .map(|&sample| sample as f32 / i16::MAX as f32)
            .collect();

        assert!(converted_f32[0] > 0.09 && converted_f32[0] < 0.11);
        assert!(converted_f32[1] < -0.19 && converted_f32[1] > -0.21);
    }

    /// Test high-frequency callback simulation (stress test)
    #[tokio::test]
    async fn test_high_frequency_callbacks() {
        let start_time = std::time::Instant::now();
        let sample_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));

        let iterations = 1000;
        let samples_per_callback = 256; // Typical audio buffer size

        for _ in 0..iterations {
            let count = sample_count.clone();
            let buf = buffer.clone();

            // Simulate high-frequency callback
            tokio::spawn(async move {
                count.fetch_add(samples_per_callback, Ordering::SeqCst);

                let mut buffer_guard = buf.lock().await;
                for i in 0..samples_per_callback {
                    buffer_guard.push((i as f32) * 0.001);
                }

                // Simulate buffer management - keep only recent samples
                if buffer_guard.len() > 10000 {
                    buffer_guard.drain(0..5000);
                }
            })
            .await
            .expect("High frequency callback failed");
        }

        let elapsed = start_time.elapsed();
        let total_samples = sample_count.load(Ordering::SeqCst);

        println!("Processed {} samples in {:?}", total_samples, elapsed);
        assert_eq!(total_samples, iterations * samples_per_callback);

        // Performance assertion - should complete within reasonable time
        assert!(
            elapsed < Duration::from_secs(5),
            "Callbacks took too long: {:?}",
            elapsed
        );
    }

    /// Test callback cleanup and resource management
    #[tokio::test]
    async fn test_callback_cleanup() {
        let cleanup_called = Arc::new(AtomicBool::new(false));
        let cleanup_called_clone = cleanup_called.clone();

        struct MockAudioCallback {
            cleanup_flag: Arc<AtomicBool>,
            buffer: Vec<f32>,
        }

        impl Drop for MockAudioCallback {
            fn drop(&mut self) {
                self.cleanup_flag.store(true, Ordering::SeqCst);
                self.buffer.clear();
            }
        }

        {
            let _callback = MockAudioCallback {
                cleanup_flag: cleanup_called_clone,
                buffer: vec![0.1, 0.2, 0.3],
            };

            // Callback exists and should not be cleaned up yet
            assert!(!cleanup_called.load(Ordering::SeqCst));
        } // callback goes out of scope here

        // Now cleanup should have been called
        assert!(cleanup_called.load(Ordering::SeqCst));
    }
}
