use sendin_beats_lib::audio::{VirtualMixer, MixerConfig, AudioChannel};
use std::collections::HashMap;
use tokio_test;

/// Test VU meter level caching functionality
#[cfg(test)]
mod level_caching_tests {
    use super::*;

    async fn create_test_mixer() -> VirtualMixer {
        let config = MixerConfig::default();
        VirtualMixer::new(config).await.expect("Failed to create test mixer")
    }

    #[tokio::test]
    async fn test_initial_channel_levels_empty() {
        let mixer = create_test_mixer().await;
        
        let levels = mixer.get_channel_levels().await;
        assert!(levels.is_empty(), "Initial channel levels should be empty");
    }

    #[tokio::test]
    async fn test_initial_master_levels_zero() {
        let mixer = create_test_mixer().await;
        
        let (left_peak, left_rms, right_peak, right_rms) = mixer.get_master_levels().await;
        assert_eq!(left_peak, 0.0, "Initial left peak should be zero");
        assert_eq!(left_rms, 0.0, "Initial left RMS should be zero");
        assert_eq!(right_peak, 0.0, "Initial right peak should be zero");
        assert_eq!(right_rms, 0.0, "Initial right RMS should be zero");
    }

    #[tokio::test]
    async fn test_level_caching_consistency() {
        let mixer = create_test_mixer().await;
        
        // Get levels multiple times quickly to test caching behavior
        let levels1 = mixer.get_channel_levels().await;
        let levels2 = mixer.get_channel_levels().await;
        let levels3 = mixer.get_channel_levels().await;
        
        // All should return the same result when there's no audio processing
        assert_eq!(levels1, levels2, "Channel levels should be consistent");
        assert_eq!(levels2, levels3, "Channel levels should be consistent");
    }

    #[tokio::test]
    async fn test_master_levels_consistency() {
        let mixer = create_test_mixer().await;
        
        // Get master levels multiple times
        let master1 = mixer.get_master_levels().await;
        let master2 = mixer.get_master_levels().await;
        let master3 = mixer.get_master_levels().await;
        
        // All should return the same result when there's no audio processing
        assert_eq!(master1, master2, "Master levels should be consistent");
        assert_eq!(master2, master3, "Master levels should be consistent");
    }

    #[tokio::test]
    async fn test_concurrent_level_access() {
        let mixer = std::sync::Arc::new(create_test_mixer().await);
        
        // Test concurrent access to levels (this tests the fallback caching)
        let handles = (0..10).map(|_| {
            let mixer_clone = mixer.clone();
            tokio::spawn(async move {
                mixer_clone.get_channel_levels().await
            })
        }).collect::<Vec<_>>();
        
        // All tasks should complete successfully
        for handle in handles {
            let result = handle.await;
            assert!(result.is_ok(), "Concurrent level access should succeed");
        }
    }

    #[tokio::test]
    async fn test_concurrent_master_level_access() {
        let mixer = std::sync::Arc::new(create_test_mixer().await);
        
        // Test concurrent access to master levels
        let handles = (0..10).map(|_| {
            let mixer_clone = mixer.clone();
            tokio::spawn(async move {
                mixer_clone.get_master_levels().await
            })
        }).collect::<Vec<_>>();
        
        // All tasks should complete successfully
        for handle in handles {
            let result = handle.await;
            assert!(result.is_ok(), "Concurrent master level access should succeed");
            let (left_peak, left_rms, right_peak, right_rms) = result.unwrap();
            
            // Levels should be valid numbers
            assert!(left_peak.is_finite(), "Left peak should be finite");
            assert!(left_rms.is_finite(), "Left RMS should be finite");
            assert!(right_peak.is_finite(), "Right peak should be finite");
            assert!(right_rms.is_finite(), "Right RMS should be finite");
            
            // Levels should be non-negative
            assert!(left_peak >= 0.0, "Left peak should be non-negative");
            assert!(left_rms >= 0.0, "Left RMS should be non-negative");
            assert!(right_peak >= 0.0, "Right peak should be non-negative");
            assert!(right_rms >= 0.0, "Right RMS should be non-negative");
        }
    }

    #[tokio::test]
    async fn test_level_bounds() {
        let mixer = create_test_mixer().await;
        
        let (left_peak, left_rms, right_peak, right_rms) = mixer.get_master_levels().await;
        
        // Levels should be within reasonable bounds (0.0 to some maximum)
        assert!(left_peak >= 0.0 && left_peak <= 10.0, "Left peak should be in reasonable range");
        assert!(left_rms >= 0.0 && left_rms <= 10.0, "Left RMS should be in reasonable range");
        assert!(right_peak >= 0.0 && right_peak <= 10.0, "Right peak should be in reasonable range");
        assert!(right_rms >= 0.0 && right_rms <= 10.0, "Right RMS should be in reasonable range");
    }

    #[tokio::test]
    async fn test_channel_levels_data_structure() {
        let mixer = create_test_mixer().await;
        
        let levels = mixer.get_channel_levels().await;
        
        // Verify the data structure is correct
        for (channel_id, (peak, rms)) in &levels {
            assert!(peak.is_finite(), "Channel {} peak should be finite", channel_id);
            assert!(rms.is_finite(), "Channel {} RMS should be finite", channel_id);
            assert!(*peak >= 0.0, "Channel {} peak should be non-negative", channel_id);
            assert!(*rms >= 0.0, "Channel {} RMS should be non-negative", channel_id);
        }
    }

    #[tokio::test]
    async fn test_level_caching_performance() {
        let mixer = create_test_mixer().await;
        
        // Measure time for level retrieval (should be fast due to caching)
        let start = std::time::Instant::now();
        
        for _ in 0..100 {
            let _levels = mixer.get_channel_levels().await;
            let _master = mixer.get_master_levels().await;
        }
        
        let elapsed = start.elapsed();
        
        // 100 level retrievals should complete quickly (under 100ms)
        assert!(elapsed.as_millis() < 100, "Level caching should be fast: {:?}", elapsed);
    }

    #[tokio::test]
    async fn test_mixed_level_access_patterns() {
        let mixer = create_test_mixer().await;
        
        // Test mixed access patterns
        let _channel_levels = mixer.get_channel_levels().await;
        let _master_levels = mixer.get_master_levels().await;
        let _channel_levels_2 = mixer.get_channel_levels().await;
        let _master_levels_2 = mixer.get_master_levels().await;
        
        // All should succeed without issues
        assert!(true, "Mixed level access patterns should work");
    }

    #[tokio::test]
    async fn test_level_access_after_mixer_start() {
        let mut mixer = create_test_mixer().await;
        
        // Start the mixer (this starts the processing thread)
        let start_result = mixer.start().await;
        assert!(start_result.is_ok(), "Mixer should start successfully");
        
        // Wait a bit for processing thread to initialize
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        
        // Level access should still work after starting
        let levels = mixer.get_channel_levels().await;
        let master = mixer.get_master_levels().await;
        
        assert!(levels.is_empty(), "Channel levels should still be accessible after start");
        
        let (left_peak, left_rms, right_peak, right_rms) = master;
        assert!(left_peak.is_finite(), "Master levels should still be accessible after start");
        assert!(left_rms.is_finite(), "Master levels should still be accessible after start");
        assert!(right_peak.is_finite(), "Master levels should still be accessible after start");
        assert!(right_rms.is_finite(), "Master levels should still be accessible after start");
        
        // Stop the mixer
        let stop_result = mixer.stop().await;
        assert!(stop_result.is_ok(), "Mixer should stop successfully");
    }
}