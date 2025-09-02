#[cfg(test)]
mod simple_tests {
    #[test]
    fn test_basic_functionality() {
        // Basic test to ensure test framework is working
        assert_eq!(2 + 2, 4);
        assert_eq!("hello".len(), 5);
        assert!(true);
    }

    #[test]
    fn test_audio_calculations() {
        // Test basic audio calculation concepts
        let samples = vec![0.5f32, -0.3f32, 0.8f32, -0.1f32];

        // Peak detection
        let peak = samples
            .iter()
            .map(|s| s.abs())
            .fold(0.0f32, |a, b| a.max(b));
        assert_eq!(peak, 0.8);

        // RMS calculation
        let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
        let rms = (sum_squares / samples.len() as f32).sqrt();
        assert!(rms > 0.0);
        assert!(rms < 1.0);

        // Verify RMS is less than or equal to peak for typical signals
        assert!(rms <= peak);
    }

    #[test]
    fn test_silence_handling() {
        let silence = vec![0.0f32; 100];

        let peak = silence
            .iter()
            .map(|s| s.abs())
            .fold(0.0f32, |a, b| a.max(b));
        assert_eq!(peak, 0.0);

        let sum_squares: f32 = silence.iter().map(|s| s * s).sum();
        let rms = (sum_squares / silence.len() as f32).sqrt();
        assert_eq!(rms, 0.0);
    }
}
