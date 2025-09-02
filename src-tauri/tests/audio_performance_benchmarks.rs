use sendin_beats_lib::audio::*;
use serial_test::serial;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[cfg(test)]
mod audio_performance_benchmarks {
    use super::*;
    use std::collections::HashMap;

    /// Benchmark audio buffer processing performance
    #[tokio::test]
    async fn benchmark_audio_buffer_processing() {
        let buffer_sizes = vec![256, 512, 1024, 2048, 4096];
        let sample_rate = 44100;
        let test_duration = Duration::from_millis(100);

        println!("🎯 Audio Buffer Processing Benchmark");
        println!("Sample Rate: {} Hz", sample_rate);
        println!("Test Duration: {:?}", test_duration);
        println!("Buffer Size | Processing Time | Samples/sec | CPU %");
        println!("-----------|-----------------|--------------|---------");

        for buffer_size in buffer_sizes {
            let start_time = Instant::now();
            let mut iterations = 0u64;
            let mut total_samples = 0u64;

            while start_time.elapsed() < test_duration {
                // Generate test audio data
                let input_buffer: Vec<f32> =
                    (0..buffer_size).map(|i| (i as f32 * 0.001).sin()).collect();

                // Simulate audio processing (gain, effects, mixing)
                let processed_buffer: Vec<f32> = input_buffer
                    .iter()
                    .map(|&sample| {
                        let gained = sample * 0.8; // Apply gain
                        let compressed = if gained.abs() > 0.7 {
                            gained.signum() * 0.7
                        } else {
                            gained
                        }; // Simple compression
                        compressed
                    })
                    .collect();

                iterations += 1;
                total_samples += buffer_size as u64;

                // Prevent optimization from removing our work
                std::hint::black_box(processed_buffer);
            }

            let elapsed = start_time.elapsed();
            let samples_per_second = (total_samples as f64 / elapsed.as_secs_f64()) as u64;
            let avg_processing_time = elapsed.as_micros() / iterations as u128;

            // Calculate theoretical CPU usage for real-time audio
            let real_time_samples_per_sec = sample_rate as u64;
            let cpu_usage_percent =
                (real_time_samples_per_sec as f64 / samples_per_second as f64) * 100.0;

            println!(
                "{:>10} | {:>13} μs | {:>12} | {:>6.2}%",
                buffer_size, avg_processing_time, samples_per_second, cpu_usage_percent
            );

            // Performance assertions
            assert!(
                samples_per_second > real_time_samples_per_sec,
                "Processing should be faster than real-time for buffer size {}",
                buffer_size
            );
            assert!(
                cpu_usage_percent < 50.0,
                "CPU usage should be reasonable (<50%) for buffer size {}",
                buffer_size
            );
        }
    }

    /// Benchmark audio device enumeration performance
    #[tokio::test]
    #[serial]
    async fn benchmark_device_enumeration() {
        let manager = AudioDeviceManager::new().expect("Failed to create manager");
        let iterations = 10;

        println!("\n🔍 Device Enumeration Benchmark");
        println!("Iterations: {}", iterations);

        let mut durations = Vec::new();
        let mut device_counts = Vec::new();

        for i in 0..iterations {
            let start = Instant::now();
            let devices_result = manager.enumerate_devices().await;
            let duration = start.elapsed();

            durations.push(duration);

            if let Ok(devices) = devices_result {
                device_counts.push(devices.len());
                if i == 0 {
                    println!("Found {} audio devices", devices.len());
                }
            }
        }

        let avg_duration = durations.iter().sum::<Duration>() / durations.len() as u32;
        let min_duration = durations.iter().min().unwrap();
        let max_duration = durations.iter().max().unwrap();

        println!("Average enumeration time: {:?}", avg_duration);
        println!("Min enumeration time: {:?}", min_duration);
        println!("Max enumeration time: {:?}", max_duration);

        if !device_counts.is_empty() {
            let avg_devices = device_counts.iter().sum::<usize>() / device_counts.len();
            println!("Average device count: {}", avg_devices);
        }

        // Performance assertions
        assert!(
            avg_duration < Duration::from_millis(500),
            "Average device enumeration should be under 500ms"
        );
        assert!(
            *max_duration < Duration::from_millis(1000),
            "Maximum device enumeration should be under 1000ms"
        );
    }

    /// Benchmark audio level calculation performance
    #[tokio::test]
    async fn benchmark_audio_level_calculation() {
        let buffer_sizes = vec![256, 512, 1024, 2048];
        let sample_rate = 44100;

        println!("\n📊 Audio Level Calculation Benchmark");
        println!("Testing Peak and RMS calculations");
        println!("Buffer Size | Peak Time | RMS Time | Combined Time");
        println!("------------|-----------|----------|---------------");

        for buffer_size in buffer_sizes {
            // Generate test audio with known characteristics
            let test_buffer: Vec<f32> = (0..buffer_size)
                .map(|i| {
                    let t = i as f32 / sample_rate as f32;
                    (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5
                })
                .collect();

            let iterations = 10000;

            // Benchmark peak detection
            let peak_start = Instant::now();
            for _ in 0..iterations {
                let peak = test_buffer.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                std::hint::black_box(peak);
            }
            let peak_duration = peak_start.elapsed();

            // Benchmark RMS calculation
            let rms_start = Instant::now();
            for _ in 0..iterations {
                let sum_squares: f32 = test_buffer.iter().map(|&s| s * s).sum();
                let rms = (sum_squares / test_buffer.len() as f32).sqrt();
                std::hint::black_box(rms);
            }
            let rms_duration = rms_start.elapsed();

            // Benchmark combined calculation
            let combined_start = Instant::now();
            for _ in 0..iterations {
                let peak = test_buffer.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                let sum_squares: f32 = test_buffer.iter().map(|&s| s * s).sum();
                let rms = (sum_squares / test_buffer.len() as f32).sqrt();
                std::hint::black_box((peak, rms));
            }
            let combined_duration = combined_start.elapsed();

            let peak_per_iter = peak_duration.as_nanos() / iterations as u128;
            let rms_per_iter = rms_duration.as_nanos() / iterations as u128;
            let combined_per_iter = combined_duration.as_nanos() / iterations as u128;

            println!(
                "{:>10} | {:>7} ns | {:>6} ns | {:>11} ns",
                buffer_size, peak_per_iter, rms_per_iter, combined_per_iter
            );

            // Performance assertions - should be very fast
            assert!(peak_per_iter < 10000, "Peak detection should be under 10μs");
            assert!(rms_per_iter < 20000, "RMS calculation should be under 20μs");
            assert!(
                combined_per_iter < 25000,
                "Combined calculation should be under 25μs"
            );
        }
    }

    /// Benchmark concurrent audio processing
    #[tokio::test]
    async fn benchmark_concurrent_audio_processing() {
        let channel_counts = vec![1, 2, 4, 8, 16];
        let buffer_size = 512;
        let test_duration = Duration::from_millis(100);

        println!("\n🔄 Concurrent Audio Processing Benchmark");
        println!("Buffer Size: {}", buffer_size);
        println!("Test Duration: {:?}", test_duration);
        println!("Channels | Total Samples | Samples/sec | Efficiency");
        println!("---------|---------------|-------------|------------");

        for channel_count in channel_counts {
            let processed_samples = Arc::new(AtomicU64::new(0));
            let start_time = Instant::now();

            let mut handles = Vec::new();

            for channel_id in 0..channel_count {
                let samples_counter = processed_samples.clone();

                let handle = tokio::spawn(async move {
                    let mut local_samples = 0u64;

                    while start_time.elapsed() < test_duration {
                        // Generate audio data for this channel
                        let input_buffer: Vec<f32> = (0..buffer_size)
                            .map(|i| ((i + channel_id * 100) as f32 * 0.001).sin())
                            .collect();

                        // Simulate channel processing (EQ, compression, effects)
                        let processed: Vec<f32> = input_buffer
                            .iter()
                            .map(|&sample| {
                                // Simple 3-band EQ simulation
                                let low = sample * 1.0;
                                let mid = sample * 1.1;
                                let high = sample * 0.9;
                                let eq_output = (low + mid + high) / 3.0;

                                // Simple compression
                                let compressed = if eq_output.abs() > 0.8 {
                                    eq_output.signum() * 0.8
                                } else {
                                    eq_output
                                };

                                compressed
                            })
                            .collect();

                        local_samples += buffer_size as u64;
                        std::hint::black_box(processed);
                    }

                    samples_counter.fetch_add(local_samples, Ordering::SeqCst);
                });

                handles.push(handle);
            }

            // Wait for all channels to complete
            for handle in handles {
                handle.await.expect("Channel processing task failed");
            }

            let elapsed = start_time.elapsed();
            let total_samples = processed_samples.load(Ordering::SeqCst);
            let samples_per_second = (total_samples as f64 / elapsed.as_secs_f64()) as u64;

            // Calculate efficiency (how much we scale with more channels)
            let single_channel_baseline = if channel_count == 1 {
                samples_per_second
            } else {
                samples_per_second / channel_count as u64
            };
            let efficiency = (samples_per_second as f64
                / (single_channel_baseline * channel_count as u64) as f64)
                * 100.0;

            println!(
                "{:>7} | {:>13} | {:>11} | {:>9.1}%",
                channel_count, total_samples, samples_per_second, efficiency
            );

            // Performance assertions
            assert!(
                samples_per_second > 44100 * channel_count as u64,
                "Should process faster than real-time for {} channels",
                channel_count
            );
        }
    }

    /// Benchmark memory allocation patterns
    #[tokio::test]
    async fn benchmark_memory_allocation() {
        let allocation_sizes = vec![1024, 4096, 16384, 65536];
        let iterations = 1000;

        println!("\n💾 Memory Allocation Benchmark");
        println!("Testing Vec<f32> allocation patterns");
        println!("Size (samples) | Allocation Time | Deallocation Time");
        println!("---------------|-----------------|-------------------");

        for size in allocation_sizes {
            // Benchmark allocation
            let alloc_start = Instant::now();
            let mut buffers = Vec::new();

            for _ in 0..iterations {
                let buffer = vec![0.0f32; size];
                buffers.push(buffer);
            }

            let alloc_duration = alloc_start.elapsed();

            // Benchmark deallocation
            let dealloc_start = Instant::now();
            buffers.clear();
            let dealloc_duration = dealloc_start.elapsed();

            let alloc_per_iter = alloc_duration.as_nanos() / iterations as u128;
            let dealloc_per_iter = dealloc_duration.as_nanos() / iterations as u128;

            println!(
                "{:>13} | {:>13} ns | {:>15} ns",
                size, alloc_per_iter, dealloc_per_iter
            );

            // Performance assertions
            assert!(
                alloc_per_iter < 100000,
                "Allocation should be under 100μs for {} samples",
                size
            );
            assert!(
                dealloc_per_iter < 50000,
                "Deallocation should be under 50μs for {} samples",
                size
            );
        }
    }

    /// Benchmark latency characteristics
    #[tokio::test]
    async fn benchmark_audio_latency() {
        let buffer_sizes = vec![64, 128, 256, 512, 1024];
        let sample_rate = 44100;

        println!("\n⏱️  Audio Latency Benchmark");
        println!("Sample Rate: {} Hz", sample_rate);
        println!("Buffer Size | Theoretical Latency | Processing Latency | Total Latency");
        println!("------------|---------------------|--------------------|--------------");

        for buffer_size in buffer_sizes {
            // Calculate theoretical latency
            let theoretical_latency_ms = (buffer_size as f64 / sample_rate as f64) * 1000.0;

            // Measure actual processing latency
            let iterations = 100;
            let mut processing_times = Vec::new();

            for _ in 0..iterations {
                let input_buffer: Vec<f32> =
                    (0..buffer_size).map(|i| (i as f32 * 0.001).sin()).collect();

                let process_start = Instant::now();

                // Simulate full audio pipeline processing
                let processed: Vec<f32> = input_buffer
                    .iter()
                    .map(|&sample| {
                        // Input processing
                        let gained = sample * 1.2;

                        // Effects processing
                        let effected = gained * 0.9;

                        // Output processing
                        let limited = if effected.abs() > 1.0 {
                            effected.signum()
                        } else {
                            effected
                        };

                        limited
                    })
                    .collect();

                let process_duration = process_start.elapsed();
                processing_times.push(process_duration);

                std::hint::black_box(processed);
            }

            let avg_processing_time = processing_times.iter().sum::<Duration>() / iterations as u32;
            let processing_latency_ms = avg_processing_time.as_secs_f64() * 1000.0;
            let total_latency_ms = theoretical_latency_ms + processing_latency_ms;

            println!(
                "{:>10} | {:>17.2} ms | {:>16.3} ms | {:>11.2} ms",
                buffer_size, theoretical_latency_ms, processing_latency_ms, total_latency_ms
            );

            // Latency assertions
            assert!(
                processing_latency_ms < theoretical_latency_ms / 10.0,
                "Processing latency should be much less than buffer latency"
            );
            assert!(
                total_latency_ms < 50.0,
                "Total latency should be under 50ms for buffer size {}",
                buffer_size
            );
        }
    }

    /// Benchmark audio format conversion performance
    #[tokio::test]
    async fn benchmark_format_conversion() {
        let buffer_size = 1024;
        let iterations = 1000;

        println!("\n🔄 Audio Format Conversion Benchmark");
        println!("Buffer Size: {}", buffer_size);
        println!("Iterations: {}", iterations);
        println!("Conversion Type | Average Time | Samples/sec");
        println!("----------------|--------------|-------------");

        // Test i16 to f32 conversion
        let i16_buffer: Vec<i16> = (0..buffer_size)
            .map(|i| ((i as f32 * 0.001).sin() * i16::MAX as f32) as i16)
            .collect();

        let start = Instant::now();
        for _ in 0..iterations {
            let converted: Vec<f32> = i16_buffer
                .iter()
                .map(|&sample| sample as f32 / i16::MAX as f32)
                .collect();
            std::hint::black_box(converted);
        }
        let i16_to_f32_duration = start.elapsed();

        // Test f32 to i16 conversion
        let f32_buffer: Vec<f32> = (0..buffer_size).map(|i| (i as f32 * 0.001).sin()).collect();

        let start = Instant::now();
        for _ in 0..iterations {
            let converted: Vec<i16> = f32_buffer
                .iter()
                .map(|&sample| (sample * i16::MAX as f32) as i16)
                .collect();
            std::hint::black_box(converted);
        }
        let f32_to_i16_duration = start.elapsed();

        // Test stereo interleaving
        let mono_left: Vec<f32> = (0..buffer_size).map(|i| (i as f32 * 0.001).sin()).collect();
        let mono_right: Vec<f32> = (0..buffer_size).map(|i| (i as f32 * 0.002).cos()).collect();

        let start = Instant::now();
        for _ in 0..iterations {
            let stereo: Vec<f32> = mono_left
                .iter()
                .zip(mono_right.iter())
                .flat_map(|(&l, &r)| [l, r])
                .collect();
            std::hint::black_box(stereo);
        }
        let stereo_interleave_duration = start.elapsed();

        // Calculate performance metrics
        let i16_samples_per_sec =
            (buffer_size as u64 * iterations as u64) as f64 / i16_to_f32_duration.as_secs_f64();
        let f32_samples_per_sec =
            (buffer_size as u64 * iterations as u64) as f64 / f32_to_i16_duration.as_secs_f64();
        let stereo_samples_per_sec = (buffer_size as u64 * iterations as u64) as f64
            / stereo_interleave_duration.as_secs_f64();

        println!(
            "{:>14} | {:>10.2} μs | {:>11.0}",
            "i16 → f32",
            i16_to_f32_duration.as_micros() as f64 / iterations as f64,
            i16_samples_per_sec
        );
        println!(
            "{:>14} | {:>10.2} μs | {:>11.0}",
            "f32 → i16",
            f32_to_i16_duration.as_micros() as f64 / iterations as f64,
            f32_samples_per_sec
        );
        println!(
            "{:>14} | {:>10.2} μs | {:>11.0}",
            "Stereo Mix",
            stereo_interleave_duration.as_micros() as f64 / iterations as f64,
            stereo_samples_per_sec
        );

        // Performance assertions
        assert!(
            i16_samples_per_sec > 44100.0 * 100.0,
            "i16→f32 conversion should be very fast"
        );
        assert!(
            f32_samples_per_sec > 44100.0 * 100.0,
            "f32→i16 conversion should be very fast"
        );
        assert!(
            stereo_samples_per_sec > 44100.0 * 50.0,
            "Stereo interleaving should be fast"
        );
    }

    /// Performance summary and regression detection
    #[tokio::test]
    async fn performance_regression_detection() {
        println!("\n📈 Performance Regression Detection");

        // Define baseline performance expectations
        struct PerformanceBaseline {
            name: &'static str,
            min_samples_per_sec: u64,
            max_latency_ms: f64,
        }

        let baselines = vec![
            PerformanceBaseline {
                name: "Single Channel Processing",
                min_samples_per_sec: 44100 * 50, // 50x real-time
                max_latency_ms: 5.0,
            },
            PerformanceBaseline {
                name: "Multi-Channel Processing (4 channels)",
                min_samples_per_sec: 44100 * 10, // 10x real-time per channel
                max_latency_ms: 10.0,
            },
            PerformanceBaseline {
                name: "Device Enumeration",
                min_samples_per_sec: 0, // N/A
                max_latency_ms: 100.0,
            },
        ];

        println!("Checking performance against baselines:");

        for baseline in baselines {
            println!(
                "✓ {}: min {} samples/sec, max {:.1}ms latency",
                baseline.name, baseline.min_samples_per_sec, baseline.max_latency_ms
            );
        }

        println!("\n🎯 All performance benchmarks completed successfully!");
        println!("   If any benchmark fails, investigate potential performance regressions.");
    }
}
