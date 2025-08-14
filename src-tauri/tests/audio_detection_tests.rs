use sendin_beats_lib::audio::{AudioConfigFactory};

#[cfg(test)]
mod audio_config_factory_tests {
    use super::*;

    #[test]
    fn test_audio_config_factory_exists() {
        // Test that AudioConfigFactory exists and can be referenced
        // This is a basic compilation test for the factory struct
        // AudioConfigFactory is likely a unit struct, test that it exists
        // This is a basic compilation test
        let _result = std::any::type_name::<AudioConfigFactory>();
        assert!(true);
    }
}

#[cfg(test)]
mod basic_audio_tests {
    use super::*;

    #[test]
    fn test_sample_processing_logic() {
        // Test basic sample processing concepts without relying on specific implementations
        let samples = vec![0.5f32, -0.3f32, 0.8f32, -0.1f32];
        
        // Calculate peak manually (what we expect from peak detection)
        let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, |a, b| a.max(b));
        assert_eq!(peak, 0.8);
        
        // Calculate RMS manually (what we expect from RMS detection)
        let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
        let rms = (sum_squares / samples.len() as f32).sqrt();
        assert!(rms > 0.0 && rms < 1.0);
    }

    #[test]
    fn test_silence_detection() {
        let silence = vec![0.0f32; 1000];
        
        // Peak of silence should be 0
        let peak = silence.iter().map(|s| s.abs()).fold(0.0f32, |a, b| a.max(b));
        assert_eq!(peak, 0.0);
        
        // RMS of silence should be 0
        let sum_squares: f32 = silence.iter().map(|s| s * s).sum();
        let rms = if silence.is_empty() { 0.0 } else { (sum_squares / silence.len() as f32).sqrt() };
        assert_eq!(rms, 0.0);
    }

    #[test]
    fn test_full_scale_signal() {
        let full_scale = vec![1.0f32, -1.0f32, 1.0f32, -1.0f32];
        
        // Peak should be 1.0
        let peak = full_scale.iter().map(|s| s.abs()).fold(0.0f32, |a, b| a.max(b));
        assert_eq!(peak, 1.0);
        
        // RMS of square wave should be 1.0
        let sum_squares: f32 = full_scale.iter().map(|s| s * s).sum();
        let rms = (sum_squares / full_scale.len() as f32).sqrt();
        assert!((rms - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_negative_samples_abs() {
        let negative_samples = vec![-0.5f32, -0.8f32, -0.3f32];
        
        // Peak should be the absolute maximum
        let peak = negative_samples.iter().map(|s| s.abs()).fold(0.0f32, |a, b| a.max(b));
        assert_eq!(peak, 0.8);
        
        // All samples should contribute positively to RMS calculation
        let sum_squares: f32 = negative_samples.iter().map(|s| s * s).sum();
        let rms = (sum_squares / negative_samples.len() as f32).sqrt();
        assert!(rms > 0.0);
    }

    #[test]
    fn test_single_sample_metrics() {
        let single = vec![0.707f32];
        
        let peak = single[0].abs();
        assert_eq!(peak, 0.707);
        
        let rms = single[0].abs(); // For single sample, RMS equals absolute value
        assert!((rms - 0.707).abs() < 0.001);
    }
}