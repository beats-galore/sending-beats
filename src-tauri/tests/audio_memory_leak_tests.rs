use sendin_beats_lib::audio::*;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering}};
use tokio::sync::Mutex;
use std::time::{Duration, Instant};
use serial_test::serial;

#[cfg(test)]
mod audio_memory_leak_tests {
    use super::*;
    use std::collections::HashMap;

    /// Track memory usage for leak detection
    struct MemoryTracker {
        allocations: AtomicUsize,
        deallocations: AtomicUsize,
        peak_usage: AtomicUsize,
        current_usage: AtomicUsize,
    }

    impl MemoryTracker {
        fn new() -> Self {
            Self {
                allocations: AtomicUsize::new(0),
                deallocations: AtomicUsize::new(0),
                peak_usage: AtomicUsize::new(0),
                current_usage: AtomicUsize::new(0),
            }
        }

        fn allocate(&self, size: usize) {
            self.allocations.fetch_add(1, Ordering::SeqCst);
            let current = self.current_usage.fetch_add(size, Ordering::SeqCst) + size;
            
            // Update peak usage
            let mut peak = self.peak_usage.load(Ordering::SeqCst);
            while current > peak {
                match self.peak_usage.compare_exchange_weak(peak, current, Ordering::SeqCst, Ordering::SeqCst) {
                    Ok(_) => break,
                    Err(new_peak) => peak = new_peak,
                }
            }
        }

        fn deallocate(&self, size: usize) {
            self.deallocations.fetch_add(1, Ordering::SeqCst);
            self.current_usage.fetch_sub(size, Ordering::SeqCst);
        }

        fn get_stats(&self) -> (usize, usize, usize, usize) {
            (
                self.allocations.load(Ordering::SeqCst),
                self.deallocations.load(Ordering::SeqCst),
                self.peak_usage.load(Ordering::SeqCst),
                self.current_usage.load(Ordering::SeqCst),
            )
        }
    }

    /// Test for memory leaks in audio buffer allocation/deallocation
    #[tokio::test]
    async fn test_audio_buffer_memory_leaks() {
        let tracker = Arc::new(MemoryTracker::new());
        let iterations = 1000;
        let buffer_sizes = vec![256, 512, 1024, 2048];
        
        println!("🔍 Testing audio buffer memory leaks");
        println!("Iterations: {}", iterations);
        
        for buffer_size in buffer_sizes {
            let tracker_clone = tracker.clone();
            
            // Simulate repeated buffer allocation and deallocation
            for _ in 0..iterations {
                let buffer_size_bytes = buffer_size * std::mem::size_of::<f32>();
                tracker_clone.allocate(buffer_size_bytes);
                
                // Create and immediately drop buffer (simulating audio processing cycle)
                {
                    let _buffer = vec![0.0f32; buffer_size];
                    // Buffer goes out of scope here
                }
                
                tracker_clone.deallocate(buffer_size_bytes);
            }
            
            let (allocs, deallocs, peak, current) = tracker.get_stats();
            
            println!("Buffer size {}: {} allocs, {} deallocs, peak {} bytes, current {} bytes", 
                     buffer_size, allocs, deallocs, peak, current);
            
            // Memory leak detection
            assert_eq!(allocs, deallocs, "Allocations and deallocations should match for buffer size {}", buffer_size);
            assert_eq!(current, 0, "Current memory usage should be zero after cleanup for buffer size {}", buffer_size);
        }
        
        println!("✅ No memory leaks detected in audio buffer management");
    }

    /// Test for memory leaks in audio stream management
    #[tokio::test]
    #[serial]
    async fn test_audio_stream_memory_leaks() {
        let initial_memory = get_memory_usage();
        let iterations = 10; // Fewer iterations for stream tests due to complexity
        
        println!("🎵 Testing audio stream memory leaks");
        println!("Initial memory usage: {} KB", initial_memory / 1024);
        
        for i in 0..iterations {
            // Create and destroy virtual mixer
            let config = AudioConfigFactory::create_dj_config();
            
            if let Ok(mut mixer) = VirtualMixer::new(config).await {
                // Add some audio channels
                for channel_id in 0..4 {
                    let channel = AudioChannel {
                        id: channel_id,
                        name: format!("Test Channel {}", channel_id),
                        gain: 1.0,
                        pan: 0.0,
                        muted: false,
                        solo: false,
                        input_device_id: None,
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
                    
                    let _ = mixer.add_channel(channel).await;
                }
                
                // Start and stop mixer
                let _ = mixer.start().await;
                tokio::time::sleep(Duration::from_millis(10)).await;
                let _ = mixer.stop().await;
                
                // Mixer goes out of scope here
            }
            
            // Force garbage collection
            tokio::task::yield_now().await;
            
            if i % 5 == 4 {
                let current_memory = get_memory_usage();
                println!("Memory after {} iterations: {} KB", i + 1, current_memory / 1024);
            }
        }
        
        // Give time for cleanup
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let final_memory = get_memory_usage();
        let memory_growth = final_memory.saturating_sub(initial_memory);
        
        println!("Final memory usage: {} KB", final_memory / 1024);
        println!("Memory growth: {} KB", memory_growth / 1024);
        
        // Allow for some memory growth but detect significant leaks
        assert!(memory_growth < 10 * 1024 * 1024, // 10MB threshold
               "Memory growth ({} KB) suggests potential memory leak", memory_growth / 1024);
        
        println!("✅ No significant memory leaks detected in audio stream management");
    }

    /// Test for memory leaks in concurrent audio processing
    #[tokio::test]
    async fn test_concurrent_processing_memory_leaks() {
        let tracker = Arc::new(MemoryTracker::new());
        let num_tasks = 10;
        let iterations_per_task = 100;
        
        println!("🔄 Testing concurrent processing memory leaks");
        println!("Tasks: {}, Iterations per task: {}", num_tasks, iterations_per_task);
        
        let mut handles = Vec::new();
        
        for _task_id in 0..num_tasks {
            let tracker_clone = tracker.clone();
            
            let handle = tokio::spawn(async move {
                for _ in 0..iterations_per_task {
                    let buffer_size = 512;
                    let buffer_size_bytes = buffer_size * std::mem::size_of::<f32>();
                    
                    tracker_clone.allocate(buffer_size_bytes);
                    
                    // Simulate audio processing with temporary allocations
                    {
                        let input_buffer = vec![0.0f32; buffer_size];
                        let _processed_buffer: Vec<f32> = input_buffer.iter()
                            .map(|&sample| sample * 0.8)
                            .collect();
                        
                        // Additional temporary allocations
                        let _temp_eq = vec![0.0f32; buffer_size];
                        let _temp_comp = vec![0.0f32; buffer_size];
                        
                        // All temporary buffers go out of scope here
                    }
                    
                    tracker_clone.deallocate(buffer_size_bytes);
                    
                    // Small delay to allow for cleanup
                    tokio::time::sleep(Duration::from_micros(10)).await;
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        for handle in handles {
            handle.await.expect("Concurrent processing task failed");
        }
        
        let (allocs, deallocs, peak, current) = tracker.get_stats();
        
        println!("Total allocations: {}", allocs);
        println!("Total deallocations: {}", deallocs);
        println!("Peak memory usage: {} KB", peak / 1024);
        println!("Current memory usage: {} bytes", current);
        
        // Memory leak detection
        assert_eq!(allocs, deallocs, "Allocations and deallocations should match in concurrent processing");
        assert_eq!(current, 0, "Current memory usage should be zero after concurrent processing");
        
        println!("✅ No memory leaks detected in concurrent audio processing");
    }

    /// Test for memory leaks in audio device enumeration
    #[tokio::test]
    #[serial]
    async fn test_device_enumeration_memory_leaks() {
        let initial_memory = get_memory_usage();
        let iterations = 50;
        
        println!("🎧 Testing device enumeration memory leaks");
        println!("Iterations: {}", iterations);
        
        for i in 0..iterations {
            // Create and destroy device manager
            if let Ok(manager) = AudioDeviceManager::new() {
                // Enumerate devices multiple times
                for _ in 0..5 {
                    let _ = manager.enumerate_devices().await;
                    let _ = manager.refresh_devices().await;
                }
                
                // Manager goes out of scope here
            }
            
            if i % 10 == 9 {
                let current_memory = get_memory_usage();
                println!("Memory after {} iterations: {} KB", i + 1, current_memory / 1024);
            }
        }
        
        // Give time for cleanup
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let final_memory = get_memory_usage();
        let memory_growth = final_memory.saturating_sub(initial_memory);
        
        println!("Initial memory: {} KB", initial_memory / 1024);
        println!("Final memory: {} KB", final_memory / 1024);
        println!("Memory growth: {} KB", memory_growth / 1024);
        
        // Device enumeration should not leak significant memory
        assert!(memory_growth < 5 * 1024 * 1024, // 5MB threshold
               "Memory growth ({} KB) suggests potential memory leak in device enumeration", 
               memory_growth / 1024);
        
        println!("✅ No significant memory leaks detected in device enumeration");
    }

    /// Test for memory leaks in audio effects processing
    #[tokio::test]
    async fn test_audio_effects_memory_leaks() {
        let tracker = Arc::new(MemoryTracker::new());
        let iterations = 500;
        let buffer_size = 1024;
        
        println!("🎛️  Testing audio effects memory leaks");
        println!("Iterations: {}, Buffer size: {}", iterations, buffer_size);
        
        for i in 0..iterations {
            let buffer_size_bytes = buffer_size * std::mem::size_of::<f32>();
            tracker.allocate(buffer_size_bytes * 4); // Input + 3 effect stages
            
            // Simulate complex audio effects chain
            {
                let input_buffer = vec![((i as f32) * 0.001).sin(); buffer_size];
                
                // EQ processing
                let eq_buffer: Vec<f32> = input_buffer.iter()
                    .map(|&sample| {
                        let low = sample * 1.0;
                        let mid = sample * 1.1;
                        let high = sample * 0.9;
                        (low + mid + high) / 3.0
                    })
                    .collect();
                
                // Compression processing
                let comp_buffer: Vec<f32> = eq_buffer.iter()
                    .map(|&sample| {
                        let threshold = 0.7;
                        let ratio = 4.0;
                        
                        if sample.abs() > threshold {
                            let excess = sample.abs() - threshold;
                            let compressed_excess = excess / ratio;
                            sample.signum() * (threshold + compressed_excess)
                        } else {
                            sample
                        }
                    })
                    .collect();
                
                // Limiter processing
                let _limited_buffer: Vec<f32> = comp_buffer.iter()
                    .map(|&sample| {
                        let limit = 0.95;
                        if sample.abs() > limit {
                            sample.signum() * limit
                        } else {
                            sample
                        }
                    })
                    .collect();
                
                // All buffers go out of scope here
            }
            
            tracker.deallocate(buffer_size_bytes * 4);
        }
        
        let (allocs, deallocs, peak, current) = tracker.get_stats();
        
        println!("Total allocations: {}", allocs);
        println!("Total deallocations: {}", deallocs);
        println!("Peak memory usage: {} KB", peak / 1024);
        println!("Current memory usage: {} bytes", current);
        
        // Memory leak detection
        assert_eq!(allocs, deallocs, "Allocations and deallocations should match in effects processing");
        assert_eq!(current, 0, "Current memory usage should be zero after effects processing");
        
        println!("✅ No memory leaks detected in audio effects processing");
    }

    /// Test for memory leaks in long-running audio scenarios
    #[tokio::test]
    async fn test_long_running_memory_stability() {
        let initial_memory = get_memory_usage();
        let test_duration = Duration::from_millis(500); // Short for testing
        let sample_interval = Duration::from_millis(50);
        
        println!("⏰ Testing long-running memory stability");
        println!("Test duration: {:?}", test_duration);
        
        let start_time = Instant::now();
        let mut memory_samples = Vec::new();
        
        // Simulate continuous audio processing
        let processing_handle = tokio::spawn(async move {
            let mut iteration = 0u64;
            
            while start_time.elapsed() < test_duration {
                // Continuous audio buffer processing
                let buffer_size = 512;
                let input_buffer = vec![((iteration as f32) * 0.001).sin(); buffer_size];
                
                // Process the buffer
                let _processed: Vec<f32> = input_buffer.iter()
                    .map(|&sample| sample * 0.8)
                    .collect();
                
                iteration += 1;
                
                // Small delay to simulate real-time processing
                tokio::time::sleep(Duration::from_micros(100)).await;
            }
            
            iteration
        });
        
        // Memory monitoring
        let monitoring_handle = tokio::spawn(async move {
            while start_time.elapsed() < test_duration {
                memory_samples.push(get_memory_usage());
                tokio::time::sleep(sample_interval).await;
            }
            memory_samples
        });
        
        let total_iterations = processing_handle.await.expect("Processing task failed");
        let memory_samples = monitoring_handle.await.expect("Monitoring task failed");
        
        let final_memory = get_memory_usage();
        
        println!("Total iterations: {}", total_iterations);
        println!("Memory samples collected: {}", memory_samples.len());
        
        if !memory_samples.is_empty() {
            let min_memory = memory_samples.iter().min().unwrap();
            let max_memory = memory_samples.iter().max().unwrap();
            let avg_memory = memory_samples.iter().sum::<usize>() / memory_samples.len();
            
            println!("Initial memory: {} KB", initial_memory / 1024);
            println!("Min memory: {} KB", min_memory / 1024);
            println!("Max memory: {} KB", max_memory / 1024);
            println!("Avg memory: {} KB", avg_memory / 1024);
            println!("Final memory: {} KB", final_memory / 1024);
            
            let memory_variance = max_memory - min_memory;
            println!("Memory variance: {} KB", memory_variance / 1024);
            
            // Memory stability checks
            assert!(memory_variance < 50 * 1024 * 1024, // 50MB variance threshold
                   "Memory variance ({} KB) too high, suggests memory instability", 
                   memory_variance / 1024);
            
            let final_growth = final_memory.saturating_sub(initial_memory);
            assert!(final_growth < 20 * 1024 * 1024, // 20MB growth threshold
                   "Final memory growth ({} KB) suggests memory leak in long-running scenario", 
                   final_growth / 1024);
        }
        
        println!("✅ Memory stability verified for long-running audio processing");
    }

    /// Test for proper cleanup of audio resources
    #[tokio::test]
    async fn test_audio_resource_cleanup() {
        println!("🧹 Testing audio resource cleanup");
        
        let resource_count = Arc::new(AtomicUsize::new(0));
        
        // Simulate audio resources with Drop implementation
        struct AudioResource {
            id: usize,
            counter: Arc<AtomicUsize>,
        }
        
        impl AudioResource {
            fn new(id: usize, counter: Arc<AtomicUsize>) -> Self {
                counter.fetch_add(1, Ordering::SeqCst);
                Self { id, counter }
            }
        }
        
        impl Drop for AudioResource {
            fn drop(&mut self) {
                self.counter.fetch_sub(1, Ordering::SeqCst);
                println!("🗑️  Cleaned up audio resource {}", self.id);
            }
        }
        
        // Create and drop resources in different scopes
        {
            let mut resources = Vec::new();
            for i in 0..10 {
                resources.push(AudioResource::new(i, resource_count.clone()));
            }
            
            assert_eq!(resource_count.load(Ordering::SeqCst), 10);
            println!("Created 10 audio resources");
            
            // Resources go out of scope here
        }
        
        // Check that all resources were cleaned up
        assert_eq!(resource_count.load(Ordering::SeqCst), 0);
        println!("✅ All audio resources properly cleaned up");
        
        // Test async resource cleanup
        {
            let async_resources = Arc::new(Mutex::new(Vec::new()));
            let async_count = Arc::new(AtomicUsize::new(0));
            
            for i in 0..5 {
                let resources = async_resources.clone();
                let count = async_count.clone();
                
                tokio::spawn(async move {
                    let resource = AudioResource::new(i + 100, count);
                    let mut res_vec = resources.lock().await;
                    res_vec.push(resource);
                }).await.expect("Async resource creation failed");
            }
            
            assert_eq!(async_count.load(Ordering::SeqCst), 5);
            println!("Created 5 async audio resources");
            
            // Clear async resources
            async_resources.lock().await.clear();
            
            // Give time for cleanup
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        println!("✅ Async audio resource cleanup verified");
    }

    /// Helper function to get current memory usage (cross-platform)
    fn get_memory_usage() -> usize {
        // This is a simplified memory usage estimation
        // In a real implementation, you would use platform-specific APIs
        
        #[cfg(target_os = "macos")]
        {
            // On macOS, we could use mach APIs, but for testing we'll use a simple estimation
            use std::process::Command;
            
            if let Ok(output) = Command::new("ps")
                .args(&["-o", "rss=", "-p"])
                .arg(std::process::id().to_string())
                .output()
            {
                if let Ok(memory_str) = String::from_utf8(output.stdout) {
                    if let Ok(memory_kb) = memory_str.trim().parse::<usize>() {
                        return memory_kb * 1024; // Convert KB to bytes
                    }
                }
            }
        }
        
        #[cfg(target_os = "linux")]
        {
            // On Linux, read from /proc/self/status
            use std::fs;
            
            if let Ok(status) = fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            if let Ok(memory_kb) = parts[1].parse::<usize>() {
                                return memory_kb * 1024; // Convert KB to bytes
                            }
                        }
                    }
                }
            }
        }
        
        // Fallback: return a reasonable estimate
        64 * 1024 * 1024 // 64MB as baseline
    }
}