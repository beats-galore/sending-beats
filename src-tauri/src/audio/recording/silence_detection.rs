// Audio analysis and silence detection for smart recording features
//
// This module provides silence detection, auto-stop functionality, and
// audio level analysis for intelligent recording management. It helps
// implement features like automatic stopping on silence and quality monitoring.

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tracing::{debug, info};

use super::types::RecordingConfig;

/// Silence detector for automatic recording control
pub struct SilenceDetector {
    threshold_db: f32,
    duration_threshold: Duration,
    sample_rate: u32,

    // State tracking
    recent_levels: VecDeque<f32>,
    silence_start: Option<Instant>,
    is_in_silence: bool,

    // Analysis window
    window_size_samples: usize,
    samples_since_update: usize,
}

impl SilenceDetector {
    /// Create a new silence detector
    pub fn new(threshold_db: f32, duration_sec: f32, sample_rate: u32) -> Self {
        let duration_threshold = Duration::from_secs_f32(duration_sec);
        let window_size_samples = (sample_rate as f32 * 0.1) as usize; // 100ms analysis window

        Self {
            threshold_db,
            duration_threshold,
            sample_rate,
            recent_levels: VecDeque::with_capacity(100), // Keep recent level history
            silence_start: None,
            is_in_silence: false,
            window_size_samples,
            samples_since_update: 0,
        }
    }

    /// Create silence detector from recording config
    pub fn from_config(config: &RecordingConfig) -> Option<Self> {
        if config.auto_stop_on_silence {
            Some(Self::new(
                config.silence_threshold_db,
                config.silence_duration_sec,
                config.sample_rate,
            ))
        } else {
            None
        }
    }

    /// Process audio samples and update silence detection state
    pub fn process_samples(&mut self, samples: &[f32]) -> SilenceAnalysis {
        if samples.is_empty() {
            return self.get_current_analysis();
        }

        // Calculate RMS level for the samples
        let rms_level = self.calculate_rms_level(samples);
        let db_level = self.linear_to_db(rms_level);

        // Update analysis window
        self.samples_since_update += samples.len();

        // Only update state at regular intervals to avoid excessive processing
        if self.samples_since_update >= self.window_size_samples {
            self.update_silence_state(db_level);
            self.samples_since_update = 0;
        }

        // Store recent level for analysis
        self.recent_levels.push_back(db_level);
        if self.recent_levels.len() > 100 {
            self.recent_levels.pop_front();
        }

        self.get_current_analysis()
    }

    /// Update silence detection state based on current level
    fn update_silence_state(&mut self, db_level: f32) {
        let now = Instant::now();
        let is_silent = db_level < self.threshold_db;

        if is_silent && !self.is_in_silence {
            // Started silence period
            self.silence_start = Some(now);
            self.is_in_silence = true;
            debug!(
                "Silence detected: {:.1} dB < {:.1} dB threshold",
                db_level, self.threshold_db
            );
        } else if !is_silent && self.is_in_silence {
            // Ended silence period
            self.silence_start = None;
            self.is_in_silence = false;
            debug!(
                "Audio resumed: {:.1} dB >= {:.1} dB threshold",
                db_level, self.threshold_db
            );
        }
    }

    /// Get current silence analysis
    fn get_current_analysis(&self) -> SilenceAnalysis {
        let should_stop = if let Some(silence_start) = self.silence_start {
            silence_start.elapsed() >= self.duration_threshold
        } else {
            false
        };

        let silence_duration = self
            .silence_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);

        let current_level_db = self.recent_levels.back().copied().unwrap_or(-100.0);
        let peak_level_db = self
            .recent_levels
            .iter()
            .fold(-100.0f32, |acc, &level| acc.max(level));
        let average_level_db = if !self.recent_levels.is_empty() {
            self.recent_levels.iter().sum::<f32>() / self.recent_levels.len() as f32
        } else {
            -100.0
        };

        SilenceAnalysis {
            is_silent: self.is_in_silence,
            should_auto_stop: should_stop,
            current_level_db,
            peak_level_db,
            average_level_db,
            silence_duration,
            threshold_db: self.threshold_db,
        }
    }

    /// Calculate RMS (Root Mean Square) level of audio samples
    fn calculate_rms_level(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }

        let sum_squares: f32 = samples.iter().map(|&sample| sample * sample).sum();
        (sum_squares / samples.len() as f32).sqrt()
    }

    /// Convert linear audio level to decibels
    fn linear_to_db(&self, linear: f32) -> f32 {
        if linear <= 0.0 {
            -100.0 // Use -100 dB as silence floor instead of negative infinity
        } else {
            20.0 * linear.log10()
        }
    }

    /// Update threshold settings
    pub fn update_threshold(&mut self, threshold_db: f32, duration_sec: f32) {
        self.threshold_db = threshold_db;
        self.duration_threshold = Duration::from_secs_f32(duration_sec);

        info!(
            "Updated silence detection: {:.1} dB threshold, {:.1}s duration",
            threshold_db, duration_sec
        );
    }

    /// Reset silence detection state
    pub fn reset(&mut self) {
        self.recent_levels.clear();
        self.silence_start = None;
        self.is_in_silence = false;
        self.samples_since_update = 0;
        debug!("Silence detector reset");
    }

    /// Get detection statistics
    pub fn get_statistics(&self) -> SilenceDetectorStats {
        let total_samples = self.recent_levels.len();
        let silent_samples = self
            .recent_levels
            .iter()
            .filter(|&&level| level < self.threshold_db)
            .count();

        let silence_percentage = if total_samples > 0 {
            (silent_samples as f32 / total_samples as f32) * 100.0
        } else {
            0.0
        };

        SilenceDetectorStats {
            threshold_db: self.threshold_db,
            duration_threshold_sec: self.duration_threshold.as_secs_f32(),
            sample_rate: self.sample_rate,
            is_currently_silent: self.is_in_silence,
            silence_percentage,
            samples_analyzed: total_samples,
        }
    }
}

/// Analysis result from silence detection
#[derive(Debug, Clone)]
pub struct SilenceAnalysis {
    pub is_silent: bool,
    pub should_auto_stop: bool,
    pub current_level_db: f32,
    pub peak_level_db: f32,
    pub average_level_db: f32,
    pub silence_duration: Duration,
    pub threshold_db: f32,
}

impl SilenceAnalysis {
    /// Get silence duration in seconds
    pub fn silence_duration_seconds(&self) -> f32 {
        self.silence_duration.as_secs_f32()
    }

    /// Check if current level is above threshold
    pub fn is_above_threshold(&self) -> bool {
        self.current_level_db >= self.threshold_db
    }

    /// Get signal-to-noise ratio estimate
    pub fn get_signal_to_noise_ratio(&self) -> f32 {
        if self.average_level_db > self.threshold_db {
            self.peak_level_db - self.threshold_db
        } else {
            0.0
        }
    }
}

/// Statistics about silence detector performance
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SilenceDetectorStats {
    pub threshold_db: f32,
    pub duration_threshold_sec: f32,
    pub sample_rate: u32,
    pub is_currently_silent: bool,
    pub silence_percentage: f32,
    pub samples_analyzed: usize,
}

/// Audio quality analyzer for recording monitoring
pub struct AudioQualityAnalyzer {
    sample_rate: u32,
    recent_peaks: VecDeque<f32>,
    recent_rms: VecDeque<f32>,
    clip_count: u32,
    total_samples: u64,
}

impl AudioQualityAnalyzer {
    /// Create a new audio quality analyzer
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            recent_peaks: VecDeque::with_capacity(100),
            recent_rms: VecDeque::with_capacity(100),
            clip_count: 0,
            total_samples: 0,
        }
    }

    /// Analyze audio samples for quality metrics
    pub fn analyze_samples(&mut self, samples: &[f32]) -> AudioQuality {
        if samples.is_empty() {
            return self.get_current_quality();
        }

        // Calculate peak and RMS levels
        let peak = samples.iter().map(|&s| s.abs()).fold(0.0, f32::max);
        let rms = (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();

        // Count clipping (samples at or above 0.99)
        let clips = samples.iter().filter(|&&s| s.abs() >= 0.99).count();
        self.clip_count += clips as u32;

        // Store recent values
        self.recent_peaks.push_back(peak);
        self.recent_rms.push_back(rms);

        if self.recent_peaks.len() > 100 {
            self.recent_peaks.pop_front();
        }
        if self.recent_rms.len() > 100 {
            self.recent_rms.pop_front();
        }

        self.total_samples += samples.len() as u64;

        self.get_current_quality()
    }

    /// Get current audio quality assessment
    fn get_current_quality(&self) -> AudioQuality {
        let peak_level = self.recent_peaks.back().copied().unwrap_or(0.0);
        let rms_level = self.recent_rms.back().copied().unwrap_or(0.0);

        let peak_db = if peak_level > 0.0 {
            20.0 * peak_level.log10()
        } else {
            -100.0
        };
        let rms_db = if rms_level > 0.0 {
            20.0 * rms_level.log10()
        } else {
            -100.0
        };

        let dynamic_range = peak_db - rms_db;
        let crest_factor = if rms_level > 0.0 {
            peak_level / rms_level
        } else {
            1.0
        };

        let clip_rate = if self.total_samples > 0 {
            (self.clip_count as f64 / self.total_samples as f64) * 100.0
        } else {
            0.0
        };

        // Assess overall quality
        let quality_score = self.calculate_quality_score(peak_db, dynamic_range, clip_rate);

        AudioQuality {
            peak_level_db: peak_db,
            rms_level_db: rms_db,
            dynamic_range_db: dynamic_range,
            crest_factor,
            clip_rate_percent: clip_rate,
            quality_score,
            total_samples_analyzed: self.total_samples,
        }
    }

    /// Calculate overall quality score (0-100)
    fn calculate_quality_score(&self, peak_db: f32, dynamic_range: f32, clip_rate: f64) -> f32 {
        let mut score = 100.0;

        // Penalize clipping heavily
        if clip_rate > 0.1 {
            score -= (clip_rate * 50.0) as f32; // Major penalty for clipping
        }

        // Penalize very low levels
        if peak_db < -40.0 {
            score -= (-40.0 - peak_db) * 2.0; // Penalty for low levels
        }

        // Penalize very high levels (near clipping)
        if peak_db > -3.0 {
            score -= (peak_db + 3.0) * 10.0; // Penalty for levels too close to clipping
        }

        // Penalize poor dynamic range
        if dynamic_range < 6.0 {
            score -= (6.0 - dynamic_range) * 5.0; // Penalty for compressed audio
        }

        score.max(0.0).min(100.0)
    }

    /// Reset analyzer state
    pub fn reset(&mut self) {
        self.recent_peaks.clear();
        self.recent_rms.clear();
        self.clip_count = 0;
        self.total_samples = 0;
    }
}

/// Audio quality assessment result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioQuality {
    pub peak_level_db: f32,
    pub rms_level_db: f32,
    pub dynamic_range_db: f32,
    pub crest_factor: f32,
    pub clip_rate_percent: f64,
    pub quality_score: f32, // 0-100
    pub total_samples_analyzed: u64,
}

impl AudioQuality {
    /// Check if audio quality is acceptable
    pub fn is_acceptable(&self) -> bool {
        self.quality_score >= 70.0 && self.clip_rate_percent < 0.1
    }

    /// Get quality assessment as text
    pub fn get_quality_text(&self) -> &'static str {
        if self.quality_score >= 90.0 {
            "Excellent"
        } else if self.quality_score >= 80.0 {
            "Very Good"
        } else if self.quality_score >= 70.0 {
            "Good"
        } else if self.quality_score >= 60.0 {
            "Fair"
        } else if self.quality_score >= 50.0 {
            "Poor"
        } else {
            "Very Poor"
        }
    }
}
