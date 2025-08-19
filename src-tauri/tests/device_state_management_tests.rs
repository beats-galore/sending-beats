use sendin_beats_lib::audio::{AudioDeviceManager, AudioDeviceInfo};
use serial_test::serial;
use std::collections::{HashMap, HashSet};

/// Test device state management and caching behavior
#[cfg(test)]
mod device_state_management_tests {
    use super::*;

    async fn create_test_device_manager() -> AudioDeviceManager {
        AudioDeviceManager::new().expect("Failed to create test device manager")
    }

    #[tokio::test]
    #[serial]
    async fn test_device_cache_initial_state() {
        let device_manager = create_test_device_manager().await;
        
        // Cache should be empty initially
        let non_existent = device_manager.get_device("non_existent").await;
        assert!(non_existent.is_none(), "Cache should be empty initially");
        
        // After enumeration, cache should be populated
        let devices = device_manager.enumerate_devices().await.unwrap();
        assert!(!devices.is_empty(), "Should find devices");
        
        // First device should now be in cache
        let first_device_id = &devices[0].id;
        let cached_device = device_manager.get_device(first_device_id).await;
        assert!(cached_device.is_some(), "First device should be cached after enumeration");
        
        let cached = cached_device.unwrap();
        assert_eq!(cached.id, devices[0].id, "Cached device ID should match");
        assert_eq!(cached.name, devices[0].name, "Cached device name should match");
    }

    #[tokio::test]
    #[serial]
    async fn test_device_cache_persistence() {
        let device_manager = create_test_device_manager().await;
        
        // Enumerate devices to populate cache
        let devices = device_manager.enumerate_devices().await.unwrap();
        assert!(!devices.is_empty(), "Should find devices");
        
        // Store device info for comparison
        let device_info: HashMap<String, AudioDeviceInfo> = devices.iter()
            .map(|d| (d.id.clone(), d.clone()))
            .collect();
        
        // Test cache persistence after multiple operations
        for device_id in device_info.keys().take(3) {
            let cached1 = device_manager.get_device(device_id).await;
            assert!(cached1.is_some(), "Device {} should be cached (check 1)", device_id);
            
            // Small delay
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            
            let cached2 = device_manager.get_device(device_id).await;
            assert!(cached2.is_some(), "Device {} should be cached (check 2)", device_id);
            
            // Cached data should be identical
            let c1 = cached1.unwrap();
            let c2 = cached2.unwrap();
            assert_eq!(c1.id, c2.id, "Cached device ID should be consistent");
            assert_eq!(c1.name, c2.name, "Cached device name should be consistent");
            assert_eq!(c1.is_input, c2.is_input, "Cached device input flag should be consistent");
            assert_eq!(c1.is_output, c2.is_output, "Cached device output flag should be consistent");
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_device_cache_update_behavior() {
        let device_manager = create_test_device_manager().await;
        
        // Initial enumeration
        let devices1 = device_manager.enumerate_devices().await.unwrap();
        assert!(!devices1.is_empty(), "Should find devices initially");
        
        let first_device_id = &devices1[0].id;
        let cached_initial = device_manager.get_device(first_device_id).await.unwrap();
        
        // Second enumeration (refresh)
        let devices2 = device_manager.enumerate_devices().await.unwrap();
        
        // Cache should be updated with fresh data
        let cached_after_refresh = device_manager.get_device(first_device_id).await;
        
        if cached_after_refresh.is_some() {
            let cached = cached_after_refresh.unwrap();
            // Device should still exist and have consistent data
            assert_eq!(cached.id, cached_initial.id, "Device ID should remain consistent");
            assert_eq!(cached.name, cached_initial.name, "Device name should remain consistent");
            
            // Verify the device is also in the new enumeration
            let found_in_new_enum = devices2.iter().any(|d| d.id == *first_device_id);
            assert!(found_in_new_enum, "Device should be found in new enumeration");
        } else {
            // Device was removed from system - this is also valid behavior
            println!("Device {} was removed from system during test", first_device_id);
            
            // Should not be in new enumeration either
            let found_in_new_enum = devices2.iter().any(|d| d.id == *first_device_id);
            assert!(!found_in_new_enum, "Removed device should not be in new enumeration");
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_device_cache_size_limits() {
        let device_manager = create_test_device_manager().await;
        
        // Enumerate devices multiple times to test cache behavior
        let mut all_device_ids = HashSet::new();
        
        for i in 0..5 {
            let devices = device_manager.enumerate_devices().await.unwrap();
            
            for device in devices {
                all_device_ids.insert(device.id.clone());
            }
            
            println!("Enumeration {}: found {} unique device IDs total", i + 1, all_device_ids.len());
        }
        
        // All discovered devices should be accessible from cache
        for device_id in &all_device_ids {
            let cached = device_manager.get_device(device_id).await;
            assert!(cached.is_some(), "Device {} should be accessible from cache", device_id);
        }
        
        // Cache should not grow indefinitely - test with non-existent devices
        for i in 0..10 {
            let fake_id = format!("fake_device_{}", i);
            let result = device_manager.get_device(&fake_id).await;
            assert!(result.is_none(), "Fake device {} should not be in cache", fake_id);
        }
        
        // Real devices should still be accessible
        for device_id in all_device_ids.iter().take(3) {
            let cached = device_manager.get_device(device_id).await;
            assert!(cached.is_some(), "Real device {} should still be accessible", device_id);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_device_state_consistency() {
        let device_manager = create_test_device_manager().await;
        
        let devices = device_manager.enumerate_devices().await.unwrap();
        assert!(!devices.is_empty(), "Should find devices");
        
        // Test state consistency for each device
        for device in devices {
            // Retrieve from cache
            let cached = device_manager.get_device(&device.id).await.unwrap();
            
            // All fields should match exactly
            assert_eq!(device.id, cached.id, "Device ID should match");
            assert_eq!(device.name, cached.name, "Device name should match");
            assert_eq!(device.is_input, cached.is_input, "Input flag should match");
            assert_eq!(device.is_output, cached.is_output, "Output flag should match");
            assert_eq!(device.is_default, cached.is_default, "Default flag should match");
            assert_eq!(device.host_api, cached.host_api, "Host API should match");
            
            // Arrays should match
            assert_eq!(device.supported_sample_rates, cached.supported_sample_rates, 
                "Sample rates should match for device {}", device.id);
            assert_eq!(device.supported_channels, cached.supported_channels, 
                "Channels should match for device {}", device.id);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_concurrent_cache_access() {
        let device_manager = create_test_device_manager().await;
        
        // Populate cache
        let devices = device_manager.enumerate_devices().await.unwrap();
        assert!(!devices.is_empty(), "Should find devices");
        
        let device_ids: Vec<String> = devices.iter().map(|d| d.id.clone()).collect();
        
        // Concurrent cache access
        let handles: Vec<_> = (0..10).map(|i| {
            let manager = &device_manager;
            let ids = device_ids.clone();
            async move {
                let mut results = Vec::new();
                for device_id in ids.iter().take(3) {
                    let cached = manager.get_device(device_id).await;
                    results.push((device_id.clone(), cached.is_some()));
                }
                (i, results)
            }
        }).collect();
        
        let results = futures::future::join_all(handles).await;
        
        // All concurrent accesses should succeed
        for (task_id, device_results) in results {
            for (device_id, found) in device_results {
                assert!(found, "Task {} should find device {} in cache", task_id, device_id);
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_device_state_after_enumeration_failure() {
        let device_manager = create_test_device_manager().await;
        
        // Successful enumeration first
        let devices = device_manager.enumerate_devices().await.unwrap();
        assert!(!devices.is_empty(), "Should find devices initially");
        
        let first_device_id = devices[0].id.clone();
        let cached_before = device_manager.get_device(&first_device_id).await;
        assert!(cached_before.is_some(), "Device should be cached initially");
        
        // Attempt multiple enumerations rapidly (might cause some to fail)
        let mut success_count = 0;
        let mut failure_count = 0;
        
        for _i in 0..20 {
            match device_manager.enumerate_devices().await {
                Ok(_) => success_count += 1,
                Err(_) => failure_count += 1,
            }
        }
        
        println!("Stress test: {} successes, {} failures", success_count, failure_count);
        
        // Even after potential failures, cache should still work
        let cached_after = device_manager.get_device(&first_device_id).await;
        
        // Device should either still be cached (if system is stable) or removed (if it was disconnected)
        match cached_after {
            Some(device) => {
                assert_eq!(device.id, first_device_id, "Cached device ID should match");
                println!("Device remained in cache after stress test");
            }
            None => {
                println!("Device was removed from cache (possibly disconnected during test)");
                // This is also valid behavior
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_device_metadata_integrity() {
        let device_manager = create_test_device_manager().await;
        
        let devices = device_manager.enumerate_devices().await.unwrap();
        
        // Test metadata integrity for each cached device
        for device in devices {
            let cached = device_manager.get_device(&device.id).await.unwrap();
            
            // Test deep equality of complex fields
            assert_eq!(device.supported_sample_rates.len(), cached.supported_sample_rates.len(),
                "Sample rates array length should match for device {}", device.id);
            
            for (i, (&original, &cached_rate)) in device.supported_sample_rates.iter()
                .zip(cached.supported_sample_rates.iter()).enumerate() {
                assert_eq!(original, cached_rate, 
                    "Sample rate {} should match for device {}", i, device.id);
            }
            
            assert_eq!(device.supported_channels.len(), cached.supported_channels.len(),
                "Channels array length should match for device {}", device.id);
            
            for (i, (&original, &cached_channels)) in device.supported_channels.iter()
                .zip(cached.supported_channels.iter()).enumerate() {
                assert_eq!(original, cached_channels, 
                    "Channel count {} should match for device {}", i, device.id);
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_invalidation_scenarios() {
        let device_manager = create_test_device_manager().await;
        
        // Initial state
        let devices1 = device_manager.enumerate_devices().await.unwrap();
        let initial_count = devices1.len();
        
        // Store some device IDs
        let test_device_ids: Vec<String> = devices1.iter()
            .take(3)
            .map(|d| d.id.clone())
            .collect();
        
        // Verify they're cached
        for device_id in &test_device_ids {
            let cached = device_manager.get_device(device_id).await;
            assert!(cached.is_some(), "Device {} should be initially cached", device_id);
        }
        
        // Re-enumerate (simulates refresh)
        let devices2 = device_manager.enumerate_devices().await.unwrap();
        
        // Device count should be similar (allowing for minor system changes)
        let count_diff = (devices2.len() as i32 - initial_count as i32).abs();
        assert!(count_diff <= 2, 
            "Device count should not change drastically: {} vs {}", 
            initial_count, devices2.len());
        
        // Test cache state after refresh
        for device_id in &test_device_ids {
            let cached_after_refresh = device_manager.get_device(device_id).await;
            
            match cached_after_refresh {
                Some(device) => {
                    // Device still exists
                    assert_eq!(device.id, *device_id, "Device ID should remain consistent");
                    
                    // Should also be in new enumeration
                    let found_in_enum = devices2.iter().any(|d| d.id == *device_id);
                    assert!(found_in_enum, "Cached device should be in new enumeration");
                }
                None => {
                    // Device was removed from cache (implies it was removed from system)
                    let found_in_enum = devices2.iter().any(|d| d.id == *device_id);
                    assert!(!found_in_enum, "Uncached device should not be in new enumeration");
                    
                    println!("Device {} was properly removed from cache and enumeration", device_id);
                }
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_device_cache_thread_safety() {
        use std::sync::Arc;
        use tokio::sync::Mutex as AsyncMutex;
        
        let device_manager = Arc::new(AsyncMutex::new(create_test_device_manager().await));
        
        // Populate cache
        {
            let manager = device_manager.lock().await;
            let _devices = manager.enumerate_devices().await.unwrap();
        }
        
        // Get a device ID for testing
        let test_device_id = {
            let manager = device_manager.lock().await;
            let devices = manager.enumerate_devices().await.unwrap();
            if devices.is_empty() {
                return; // Skip test if no devices
            }
            devices[0].id.clone()
        };
        
        // Concurrent cache reads
        let read_handles: Vec<_> = (0..5).map(|i| {
            let manager = device_manager.clone();
            let device_id = test_device_id.clone();
            tokio::spawn(async move {
                let device_manager = manager.lock().await;
                let result = device_manager.get_device(&device_id).await;
                (i, result.is_some())
            })
        }).collect();
        
        // Concurrent cache updates (via enumeration)
        let update_handles: Vec<_> = (0..3).map(|i| {
            let manager = device_manager.clone();
            tokio::spawn(async move {
                let device_manager = manager.lock().await;
                let result = device_manager.enumerate_devices().await;
                (i, result.is_ok())
            })
        }).collect();
        
        // Wait for all operations
        let read_results = futures::future::join_all(read_handles).await;
        let update_results = futures::future::join_all(update_handles).await;
        
        // All reads should succeed
        for (i, task_result) in read_results.into_iter().enumerate() {
            assert!(task_result.is_ok(), "Read task {} should not panic", i);
            let (task_id, found) = task_result.unwrap();
            assert!(found, "Read task {} should find the device", task_id);
        }
        
        // All updates should succeed
        for (i, task_result) in update_results.into_iter().enumerate() {
            assert!(task_result.is_ok(), "Update task {} should not panic", i);
            let (task_id, success) = task_result.unwrap();
            assert!(success, "Update task {} should succeed", task_id);
        }
    }
}

/// Test device state management edge cases
#[cfg(test)]
mod device_state_edge_cases {
    use super::*;

    async fn create_test_device_manager() -> AudioDeviceManager {
        AudioDeviceManager::new().expect("Failed to create test device manager")
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_with_duplicate_device_ids() {
        let device_manager = create_test_device_manager().await;
        
        let devices = device_manager.enumerate_devices().await.unwrap();
        
        // Check for duplicate device IDs (should not happen, but good to test)
        let mut seen_ids = HashSet::new();
        let mut duplicates = Vec::new();
        
        for device in &devices {
            if seen_ids.contains(&device.id) {
                duplicates.push(device.id.clone());
            } else {
                seen_ids.insert(device.id.clone());
            }
        }
        
        assert!(duplicates.is_empty(), 
            "Should not have duplicate device IDs: {:?}", duplicates);
        
        // All devices should be uniquely cacheable
        for device in devices {
            let cached = device_manager.get_device(&device.id).await;
            assert!(cached.is_some(), "Device {} should be cacheable", device.id);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_with_empty_device_names() {
        let device_manager = create_test_device_manager().await;
        
        let devices = device_manager.enumerate_devices().await.unwrap();
        
        // Verify no devices have empty names (which would break cache consistency)
        for device in devices {
            assert!(!device.name.is_empty(), 
                "Device {} should not have empty name", device.id);
            assert!(!device.id.is_empty(), 
                "Device should not have empty ID");
            
            // Cached version should also have non-empty name
            let cached = device_manager.get_device(&device.id).await.unwrap();
            assert!(!cached.name.is_empty(), 
                "Cached device {} should not have empty name", device.id);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_cache_consistency_with_device_changes() {
        let device_manager = create_test_device_manager().await;
        
        // Take snapshot of initial state
        let initial_devices = device_manager.enumerate_devices().await.unwrap();
        let initial_device_map: HashMap<String, AudioDeviceInfo> = initial_devices.iter()
            .map(|d| (d.id.clone(), d.clone()))
            .collect();
        
        // Wait a bit (simulate time passing)
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Re-enumerate
        let updated_devices = device_manager.enumerate_devices().await.unwrap();
        let updated_device_map: HashMap<String, AudioDeviceInfo> = updated_devices.iter()
            .map(|d| (d.id.clone(), d.clone()))
            .collect();
        
        // Test cache consistency for devices that remained
        for (device_id, initial_device) in &initial_device_map {
            if let Some(updated_device) = updated_device_map.get(device_id) {
                // Device still exists - cache should be updated
                let cached = device_manager.get_device(device_id).await.unwrap();
                
                // Core properties should remain the same
                assert_eq!(cached.id, updated_device.id, "Device ID should be consistent");
                assert_eq!(cached.name, updated_device.name, "Device name should be consistent");
                assert_eq!(cached.is_input, updated_device.is_input, "Input flag should be consistent");
                assert_eq!(cached.is_output, updated_device.is_output, "Output flag should be consistent");
                
                // Compare with initial to see what changed
                if initial_device.is_default != updated_device.is_default {
                    println!("Device {} default status changed: {} -> {}", 
                        device_id, initial_device.is_default, updated_device.is_default);
                }
            } else {
                // Device was removed - should not be in cache
                let cached = device_manager.get_device(device_id).await;
                if cached.is_some() {
                    println!("Warning: Removed device {} still in cache", device_id);
                }
            }
        }
        
        // Test cache for new devices
        for (device_id, _) in &updated_device_map {
            if !initial_device_map.contains_key(device_id) {
                // New device - should be in cache
                let cached = device_manager.get_device(device_id).await;
                assert!(cached.is_some(), "New device {} should be cached", device_id);
                println!("New device detected and cached: {}", device_id);
            }
        }
    }
}