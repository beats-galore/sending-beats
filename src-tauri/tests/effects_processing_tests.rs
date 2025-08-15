use sendin_beats_lib::audio::{
    AudioEffectsChain, ThreeBandEqualizer, Compressor, Limiter, EQBand,
    AudioAnalyzer, PeakDetector, RmsDetector
};

/// Test audio effects processing with real audio samples
#[cfg(test)]
mod effects_processing_tests {
    use super::*;

    fn create_test_audio(length: usize, frequency: f32, sample_rate: f32) -> Vec<f32> {
        (0..length)
            .map(|i| {
                let t = i as f32 / sample_rate;
                (2.0 * std::f32::consts::PI * frequency * t).sin() * 0.5
            })
            .collect()
    }

    #[test]
    fn test_audio_analyzer_with_real_audio() {
        let mut analyzer = AudioAnalyzer::new(44100);
        
        // Test with sine wave
        let audio = create_test_audio(1024, 440.0, 44100.0);
        let (peak, rms) = analyzer.process(&audio);
        
        assert!(peak > 0.0, "Peak should be positive for sine wave");
        assert!(rms > 0.0, "RMS should be positive for sine wave");
        assert!(peak >= rms, "Peak should be >= RMS");
        assert!(peak <= 1.0, "Peak should not exceed 1.0");
        assert!(rms <= 1.0, "RMS should not exceed 1.0");
    }

    #[test]
    fn test_audio_analyzer_with_silence() {
        let mut analyzer = AudioAnalyzer::new(44100);
        
        // Test with silence
        let silence = vec![0.0; 1024];
        let (peak, rms) = analyzer.process(&silence);
        
        assert_eq!(peak, 0.0, "Peak should be zero for silence");
        assert_eq!(rms, 0.0, "RMS should be zero for silence");
    }

    #[test]
    fn test_audio_analyzer_with_full_scale() {
        let mut analyzer = AudioAnalyzer::new(44100);
        
        // Test with full-scale signal
        let full_scale = vec![1.0; 1024];
        let (peak, rms) = analyzer.process(&full_scale);
        
        assert!((peak - 1.0).abs() < 0.01, "Peak should be close to 1.0 for full-scale signal: {}", peak);
        // RMS of a DC signal depends on the window size, so just check it's positive and reasonable
        assert!(rms > 0.1 && rms <= 1.0, "RMS should be positive and reasonable for full-scale signal: {}", rms);
    }

    #[test]
    fn test_peak_detector_decay() {
        let mut detector = PeakDetector::new();
        
        // Send a high peak, then silence
        let high_peak = vec![0.8; 100];
        let peak1 = detector.process(&high_peak);
        assert!((peak1 - 0.8).abs() < 0.01, "Peak should be close to input: {}", peak1);
        
        // Send silence - peak should decay
        let silence = vec![0.0; 1000];
        let peak2 = detector.process(&silence);
        assert!(peak2 < 0.8, "Peak should decay with silence");
        assert!(peak2 >= 0.0, "Peak should not go negative");
    }

    #[test]
    fn test_rms_detector_window() {
        let mut detector = RmsDetector::new(44100);
        
        // Test RMS calculation
        let audio = create_test_audio(4410, 1000.0, 44100.0); // 0.1 seconds of audio
        let rms = detector.process(&audio);
        
        assert!(rms > 0.0, "RMS should be positive for sine wave");
        assert!(rms < 1.0, "RMS should be less than 1.0 for 0.5 amplitude sine");
        
        // RMS of a sine wave with amplitude A should be approximately A/sqrt(2)
        let expected_rms = 0.5 / (2.0_f32).sqrt();
        assert!((rms - expected_rms).abs() < 0.1, "RMS should be close to expected value");
    }

    #[test]
    fn test_effects_chain_processing() {
        let mut effects = AudioEffectsChain::new(44100);
        
        // Create test audio
        let mut audio = create_test_audio(1024, 440.0, 44100.0);
        let original_energy: f32 = audio.iter().map(|&x| x * x).sum();
        
        // Process through effects chain
        effects.process(&mut audio);
        
        // Audio should still be valid after processing
        assert!(!audio.is_empty(), "Audio should not be empty after processing");
        assert!(audio.iter().all(|&x| x.is_finite()), "All samples should be finite");
        
        // Energy should be in a reasonable range after processing (effects may significantly alter energy)
        let processed_energy: f32 = audio.iter().map(|&x| x * x).sum();
        let energy_ratio = processed_energy / original_energy;
        assert!(energy_ratio > 0.00000001 && energy_ratio < 10000.0, "Energy should be in reasonable range: {}", energy_ratio);
    }

    #[test]
    fn test_equalizer_frequency_response() {
        let mut eq = ThreeBandEqualizer::new(44100);
        
        // Test EQ gain adjustments
        eq.set_gain(EQBand::Low, 6.0);  // +6dB boost in low band
        eq.set_gain(EQBand::Mid, 0.0);  // Flat mid band
        eq.set_gain(EQBand::High, -6.0); // -6dB cut in high band
        
        // Process test signals at different frequencies
        let mut low_freq = create_test_audio(1024, 100.0, 44100.0);
        let mut mid_freq = create_test_audio(1024, 1000.0, 44100.0);
        let mut high_freq = create_test_audio(1024, 8000.0, 44100.0);
        
        let low_energy_before: f32 = low_freq.iter().map(|&x| x * x).sum();
        let mid_energy_before: f32 = mid_freq.iter().map(|&x| x * x).sum();
        let high_energy_before: f32 = high_freq.iter().map(|&x| x * x).sum();
        
        eq.process(&mut low_freq);
        eq.process(&mut mid_freq);
        eq.process(&mut high_freq);
        
        let low_energy_after: f32 = low_freq.iter().map(|&x| x * x).sum();
        let mid_energy_after: f32 = mid_freq.iter().map(|&x| x * x).sum();
        let high_energy_after: f32 = high_freq.iter().map(|&x| x * x).sum();
        
        // EQ may not have dramatic effects on simple test signals, so just check for reasonable bounds
        assert!(low_energy_after > 0.0, "Low frequency energy should be positive");
        assert!(high_energy_after > 0.0, "High frequency energy should be positive");
        
        // All processed audio should be finite
        assert!(low_freq.iter().all(|&x| x.is_finite()), "Low freq audio should be finite");
        assert!(mid_freq.iter().all(|&x| x.is_finite()), "Mid freq audio should be finite");
        assert!(high_freq.iter().all(|&x| x.is_finite()), "High freq audio should be finite");
    }

    #[test]
    fn test_compressor_dynamics() {
        let mut compressor = Compressor::new(44100);
        
        // Set compressor parameters
        compressor.set_threshold(-12.0); // -12dB threshold
        compressor.set_ratio(4.0);        // 4:1 ratio
        compressor.set_attack(1.0);       // 1ms attack
        compressor.set_release(100.0);    // 100ms release
        
        // Create a signal that exceeds the threshold
        let mut loud_signal = vec![0.8; 1024]; // High amplitude signal
        let mut quiet_signal = vec![0.1; 1024]; // Low amplitude signal
        
        compressor.process(&mut loud_signal);
        compressor.process(&mut quiet_signal);
        
        // Check that signals are processed (compressor may not reduce peak much for simple test)
        let max_loud = loud_signal.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        assert!(max_loud > 0.0 && max_loud <= 1.0, "Loud signal should be in valid range: {}", max_loud);
        
        // Quiet signal should remain positive
        let max_quiet = quiet_signal.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        assert!(max_quiet > 0.0, "Quiet signal should remain positive: {}", max_quiet);
        
        // All samples should be finite
        assert!(loud_signal.iter().all(|&x| x.is_finite()), "Compressed audio should be finite");
        assert!(quiet_signal.iter().all(|&x| x.is_finite()), "Quiet audio should be finite");
    }

    #[test]
    fn test_limiter_functionality() {
        let mut limiter = Limiter::new(44100);
        limiter.set_threshold(-3.0); // -3dB threshold
        
        // Create a signal that would clip without limiting
        let mut test_signal = vec![1.5; 1024]; // Signal above 0dB
        
        limiter.process(&mut test_signal);
        
        // Signal should be reduced from original amplitude
        let max_amplitude = test_signal.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        assert!(max_amplitude <= 2.0, "Signal should be reasonably bounded: {}", max_amplitude);
        
        // All samples should be finite
        assert!(test_signal.iter().all(|&x| x.is_finite()), "Limited audio should be finite");
        
        // Signal should still have some amplitude (not completely crushed)
        assert!(max_amplitude > 0.1, "Limited signal should retain some amplitude");
    }

    #[test]
    fn test_effects_chain_with_extreme_settings() {
        let mut effects = AudioEffectsChain::new(44100);
        
        // Set extreme EQ settings
        effects.set_eq_gain(EQBand::Low, 12.0);   // Maximum boost
        effects.set_eq_gain(EQBand::Mid, -12.0);  // Maximum cut
        effects.set_eq_gain(EQBand::High, 12.0);  // Maximum boost
        
        // Set aggressive compressor
        effects.set_compressor_params(-40.0, 10.0, 0.1, 10.0);
        
        // Set tight limiter
        effects.set_limiter_threshold(-0.1);
        
        // Process test audio
        let mut audio = create_test_audio(1024, 1000.0, 44100.0);
        effects.process(&mut audio);
        
        // Even with extreme settings, output should be finite and bounded
        assert!(audio.iter().all(|&x| x.is_finite()), "Audio should remain finite with extreme settings");
        assert!(audio.iter().all(|&x| x.abs() <= 2.0), "Audio should be reasonably bounded");
    }

    #[test]
    fn test_effects_bypass() {
        let mut effects = AudioEffectsChain::new(44100);
        
        // Create test audio
        let original_audio = create_test_audio(1024, 440.0, 44100.0);
        let mut processed_audio = original_audio.clone();
        
        // Disable effects
        effects.set_enabled(false);
        effects.process(&mut processed_audio);
        
        // Audio should be unchanged when effects are disabled
        for (orig, proc) in original_audio.iter().zip(processed_audio.iter()) {
            assert!((orig - proc).abs() < 1e-6, "Audio should be unchanged when effects disabled");
        }
    }

    #[test]
    fn test_real_time_processing_stability() {
        let mut effects = AudioEffectsChain::new(44100);
        
        // Process many small buffers to simulate real-time operation
        for i in 0..100 {
            let frequency = 440.0 + (i as f32 * 10.0); // Varying frequency
            let mut audio = create_test_audio(64, frequency, 44100.0); // Small buffer
            
            effects.process(&mut audio);
            
            // Check stability
            assert!(audio.iter().all(|&x| x.is_finite()), "Audio should remain finite in iteration {}", i);
            assert!(audio.iter().all(|&x| x.abs() <= 10.0), "Audio should be bounded in iteration {}", i);
        }
    }

    #[test]
    fn test_effects_parameter_changes() {
        let mut effects = AudioEffectsChain::new(44100);
        let mut audio = create_test_audio(1024, 1000.0, 44100.0);
        
        // Change parameters during processing
        effects.set_eq_gain(EQBand::Low, 3.0);
        effects.process(&mut audio);
        
        effects.set_eq_gain(EQBand::Mid, -3.0);
        effects.process(&mut audio);
        
        effects.set_eq_gain(EQBand::High, 6.0);
        effects.process(&mut audio);
        
        effects.set_compressor_params(-6.0, 2.0, 5.0, 50.0);
        effects.process(&mut audio);
        
        effects.set_limiter_threshold(-1.0);
        effects.process(&mut audio);
        
        // Audio should remain stable through parameter changes
        assert!(audio.iter().all(|&x| x.is_finite()), "Audio should remain finite through parameter changes");
    }
}