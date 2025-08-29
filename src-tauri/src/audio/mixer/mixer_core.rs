// Core mixer implementation and coordination
//
// This module provides the remaining core mixer functionality that doesn't
// fit into the specialized modules, including device health monitoring,
// error reporting, and integration methods.

use anyhow::Result;
use std::collections::HashMap;
use tracing::{info, warn};

use super::super::devices::DeviceHealth;
use super::types::VirtualMixer;

impl VirtualMixer {
    /// Check if the mixer is currently running
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get the current mixer configuration
    pub async fn get_config(&self) -> crate::audio::types::MixerConfig {
        if let Ok(shared_config) = self.shared_config.lock() {
            shared_config.clone()
        } else {
            self.config.clone()
        }
    }

    /// Get device health information for monitoring
    pub async fn get_device_health(&self, device_id: &str) -> Option<DeviceHealth> {
        self.audio_device_manager.get_device_health(device_id).await
    }

    /// Get health information for all devices
    pub async fn get_all_device_health(&self) -> HashMap<String, DeviceHealth> {
        self.audio_device_manager.get_all_device_health().await
    }
    
    /// Report a device error from external sources (like stream callbacks)
    pub async fn report_device_error(&self, device_id: &str, error: String) {
        self.audio_device_manager.report_device_error(device_id, error).await;
    }

    /// Get timing performance summary for monitoring
    pub async fn get_timing_summary(&self) -> String {
        let timing_metrics = self.timing_metrics.lock().await;
        timing_metrics.get_performance_summary()
    }

    /// Check if audio processing timing is acceptable
    pub async fn is_timing_performance_acceptable(&self) -> bool {
        let timing_metrics = self.timing_metrics.lock().await;
        timing_metrics.is_performance_acceptable()
    }

    /// Get current audio clock information
    pub async fn get_clock_info(&self) -> ClockInfo {
        let audio_clock = self.audio_clock.lock().await;
        ClockInfo {
            samples_processed: audio_clock.get_samples_processed(),
            playback_time_seconds: audio_clock.get_playback_time_seconds(),
            sample_rate: audio_clock.get_sample_rate(),
            timing_drift_ms: audio_clock.get_timing_drift_ms(),
        }
    }

    /// Reset timing and performance metrics
    pub async fn reset_metrics(&self) {
        {
            let mut timing_metrics = self.timing_metrics.lock().await;
            timing_metrics.reset();
        }
        
        {
            let mut audio_clock = self.audio_clock.lock().await;
            audio_clock.reset();
        }
        
        info!("🔄 METRICS RESET: All timing and performance metrics reset");
    }

    /// Get comprehensive mixer status for debugging
    pub async fn get_status(&self) -> MixerStatus {
        let config = self.get_config().await;
        let stream_info = self.get_stream_info().await;
        let clock_info = self.get_clock_info().await;
        let timing_acceptable = self.is_timing_performance_acceptable().await;
        let timing_summary = self.get_timing_summary().await;
        
        MixerStatus {
            is_running: self.is_running(),
            config,
            stream_info,
            clock_info,
            timing_acceptable,
            timing_summary,
        }
    }

    /// Perform health check on all active devices
    pub async fn health_check(&self) -> Result<HealthCheckResult> {
        info!("🏥 HEALTH CHECK: Performing mixer health check");
        
        let mut issues = Vec::new();
        let mut healthy_devices = 0;
        let mut total_devices = 0;
        
        // Check all device health
        let all_device_health = self.get_all_device_health().await;
        for (device_id, health) in all_device_health {
            total_devices += 1;
            
            match health.status {
                super::super::devices::DeviceStatus::Connected => {
                    healthy_devices += 1;
                }
                super::super::devices::DeviceStatus::Disconnected => {
                    issues.push(format!("Device '{}' is disconnected", device_id));
                }
                super::super::devices::DeviceStatus::Error(ref error) => {
                    issues.push(format!("Device '{}' has error: {}", device_id, error));
                }
            }
            
            // Check for devices that should be avoided
            if health.consecutive_errors >= 3 {
                issues.push(format!("Device '{}' has {} consecutive errors", device_id, health.consecutive_errors));
            }
        }
        
        // Check timing performance
        if !self.is_timing_performance_acceptable().await {
            issues.push("Timing performance is degraded".to_string());
        }
        
        // Check if mixer is running when it should be
        let stream_info = self.get_stream_info().await;
        if stream_info.has_active_streams() && !self.is_running() {
            issues.push("Mixer has active streams but is not running".to_string());
        }
        
        let health_score = if total_devices > 0 {
            (healthy_devices as f32 / total_devices as f32) * 100.0
        } else {
            100.0
        };
        
        let result = HealthCheckResult {
            healthy: issues.is_empty(),
            health_score,
            issues,
            healthy_devices,
            total_devices,
        };
        
        if result.healthy {
            info!("✅ HEALTH CHECK: All systems healthy (score: {:.1}%)", health_score);
        } else {
            warn!("⚠️ HEALTH CHECK: Found {} issues (score: {:.1}%)", result.issues.len(), health_score);
        }
        
        Ok(result)
    }
}

/// Audio clock information for monitoring
#[derive(Debug, Clone)]
pub struct ClockInfo {
    pub samples_processed: u64,
    pub playback_time_seconds: f64,
    pub sample_rate: u32,
    pub timing_drift_ms: f64,
}

/// Comprehensive mixer status information
#[derive(Debug)]
pub struct MixerStatus {
    pub is_running: bool,
    pub config: crate::audio::types::MixerConfig,
    pub stream_info: super::stream_management::StreamInfo,
    pub clock_info: ClockInfo,
    pub timing_acceptable: bool,
    pub timing_summary: String,
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub health_score: f32, // 0-100%
    pub issues: Vec<String>,
    pub healthy_devices: usize,
    pub total_devices: usize,
}

impl HealthCheckResult {
    /// Get a human-readable health summary
    pub fn get_summary(&self) -> String {
        if self.healthy {
            format!("Healthy - {}/{} devices OK ({}%)", 
                   self.healthy_devices, self.total_devices, self.health_score as u8)
        } else {
            format!("Issues found - {}/{} devices OK ({}%), {} issues", 
                   self.healthy_devices, self.total_devices, self.health_score as u8, self.issues.len())
        }
    }
    
    /// Check if health score is above acceptable threshold
    pub fn is_acceptable(&self, threshold: f32) -> bool {
        self.health_score >= threshold
    }
}