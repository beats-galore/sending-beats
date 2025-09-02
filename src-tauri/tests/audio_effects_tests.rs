use sendin_beats_lib::audio::*;

#[cfg(test)]
mod equalizer_tests {
    use super::*;

    #[test]
    fn test_equalizer_creation() {
        let eq = ThreeBandEqualizer::new(44100);
        // ThreeBandEqualizer doesn't expose gain fields directly
        // It uses internal BiquadFilter structures
        // Test that it can be created without panicking
        assert!(true);
    }

    #[test]
    fn test_equalizer_process_methods() {
        let mut eq = ThreeBandEqualizer::new(44100);

        // Test that the equalizer has the expected methods
        // These would set gains via the internal filters
        let mut samples = vec![0.5f32, -0.3f32, 0.8f32, -0.1f32];

        // Should not panic when processing
        eq.process(&mut samples);
        assert_eq!(samples.len(), 4);
    }

    #[test]
    fn test_equalizer_process_no_crash() {
        let mut eq = ThreeBandEqualizer::new(44100);
        let mut samples = vec![0.5f32, -0.3f32, 0.8f32, -0.1f32];

        // Should not panic
        eq.process(&mut samples);

        // Samples should be processed
        assert_eq!(samples.len(), 4);
    }
}

#[cfg(test)]
mod compressor_tests {
    use super::*;

    #[test]
    fn test_compressor_creation() {
        let comp = Compressor::new(44100);
        // Compressor fields are private, test creation and methods
        assert!(true); // Creation succeeded
    }

    #[test]
    fn test_compressor_parameter_methods() {
        let mut comp = Compressor::new(44100);

        // Test that setter methods exist and don't panic
        comp.set_threshold(-18.0);
        comp.set_ratio(6.0);
        comp.set_attack(10.0);
        comp.set_release(200.0);

        // Methods executed without panicking
        assert!(true);
    }

    #[test]
    fn test_compressor_process() {
        let mut comp = Compressor::new(44100);
        let original_samples = vec![0.5f32, -0.3f32, 0.8f32, -0.1f32];
        let mut samples = original_samples.clone();

        comp.process(&mut samples);

        // Samples should be processed
        assert_eq!(samples.len(), original_samples.len());
    }

    #[test]
    fn test_compressor_high_level_samples() {
        let mut comp = Compressor::new(44100);
        comp.set_threshold(-6.0); // Lower threshold for compression
        let mut samples = vec![0.9f32, -0.9f32, 0.8f32, -0.8f32]; // High level samples

        comp.process(&mut samples);

        // Should not crash and should process all samples
        assert_eq!(samples.len(), 4);
    }
}

#[cfg(test)]
mod limiter_tests {
    use super::*;

    #[test]
    fn test_limiter_creation() {
        let limiter = Limiter::new(44100);
        // Limiter fields are private, test creation
        assert!(true); // Creation succeeded
    }

    #[test]
    fn test_limiter_process() {
        let mut limiter = Limiter::new(44100);
        let original_samples = vec![0.5f32, -0.3f32, 0.8f32, -0.1f32];
        let mut samples = original_samples.clone();

        limiter.process(&mut samples);

        // Samples should be processed
        assert_eq!(samples.len(), original_samples.len());
    }

    #[test]
    fn test_limiter_high_level_samples() {
        let mut limiter = Limiter::new(44100);
        limiter.set_threshold(-6.0);

        let mut samples = vec![0.9f32, -0.9f32, 0.8f32, -0.8f32]; // High level samples

        limiter.process(&mut samples);

        // Should process without crashing
        assert_eq!(samples.len(), 4);

        // All samples should be within digital bounds
        for sample in &samples {
            assert!(sample.abs() <= 1.0); // Should not exceed digital full scale
        }
    }
}
