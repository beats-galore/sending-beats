// Security and configuration validation for mixer operations
//
// This module provides comprehensive validation for device IDs, mixer
// configurations, and input sanitization to ensure security and robustness
// in the audio mixing system.

use anyhow::Result;
use super::super::types::MixerConfig;

/// Comprehensive device ID validation for security and robustness
pub fn validate_device_id(device_id: &str) -> Result<()> {
    // Basic empty/length checks
    if device_id.is_empty() {
        return Err(anyhow::anyhow!("Device ID cannot be empty"));
    }
    if device_id.len() > 256 {
        return Err(anyhow::anyhow!("Device ID too long: maximum 256 characters allowed, got {}", device_id.len()));
    }
    if device_id.len() < 2 {
        return Err(anyhow::anyhow!("Device ID too short: minimum 2 characters required"));
    }
    
    // Character validation - allow alphanumeric, underscore, dash, dot, and colon for device IDs
    let valid_chars = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':');
    if !device_id.chars().all(valid_chars) {
        let invalid_chars: String = device_id.chars()
            .filter(|&c| !valid_chars(c))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        return Err(anyhow::anyhow!(
            "Device ID contains invalid characters: '{}'. Only alphanumeric, underscore, dash, dot, and colon are allowed", 
            invalid_chars
        ));
    }
    
    // Pattern validation - must not start or end with special characters
    if device_id.starts_with(|c: char| !c.is_alphanumeric()) {
        return Err(anyhow::anyhow!("Device ID must start with alphanumeric character"));
    }
    if device_id.ends_with(|c: char| !c.is_alphanumeric()) {
        return Err(anyhow::anyhow!("Device ID must end with alphanumeric character"));
    }
    
    // Security checks - prevent common injection patterns
    let dangerous_patterns = ["../", "..\\", "//", "\\\\", ";;", "&&", "||"];
    for pattern in &dangerous_patterns {
        if device_id.contains(pattern) {
            return Err(anyhow::anyhow!("Device ID contains dangerous pattern: '{}'", pattern));
        }
    }
    
    Ok(())
}

/// Validate mixer configuration for security and performance
pub fn validate_config(config: &MixerConfig) -> Result<()> {
    // Sample rate validation
    if config.sample_rate < 8000 || config.sample_rate > 192000 {
        return Err(anyhow::anyhow!("Invalid sample rate: {} (must be 8000-192000 Hz)", config.sample_rate));
    }
    
    // Buffer size validation
    if config.buffer_size < 16 || config.buffer_size > 8192 {
        return Err(anyhow::anyhow!("Invalid buffer size: {} (must be 16-8192 samples)", config.buffer_size));
    }
    
    // Channel count validation
    if config.channels < 1 || config.channels > 32 {
        return Err(anyhow::anyhow!("Invalid channel count: {} (must be 1-32 channels)", config.channels));
    }
    
    // Logical validation - ensure buffer size is reasonable for sample rate
    let min_buffer_for_rate = (config.sample_rate / 1000).max(16); // At least 1ms of audio
    if config.buffer_size < min_buffer_for_rate as usize {
        return Err(anyhow::anyhow!(
            "Buffer size {} too small for sample rate {} (minimum: {})", 
            config.buffer_size, config.sample_rate, min_buffer_for_rate
        ));
    }
    
    // Performance validation - warn about potentially problematic configurations
    if config.buffer_size < 64 && config.sample_rate > 96000 {
        return Err(anyhow::anyhow!(
            "High sample rate ({}) with small buffer ({}) may cause audio dropouts", 
            config.sample_rate, config.buffer_size
        ));
    }
    
    Ok(())
}

/// Validate channel ID for mixer operations
pub fn validate_channel_id(channel_id: u32) -> Result<()> {
    if channel_id == 0 {
        return Err(anyhow::anyhow!("Channel ID cannot be zero (reserved)"));
    }
    if channel_id > 9999 {
        return Err(anyhow::anyhow!("Channel ID too large: {} (maximum: 9999)", channel_id));
    }
    Ok(())
}

/// Validate audio level values for VU meters
pub fn validate_audio_levels(peak_left: f32, rms_left: f32, peak_right: f32, rms_right: f32) -> Result<()> {
    let levels = [peak_left, rms_left, peak_right, rms_right];
    let names = ["peak_left", "rms_left", "peak_right", "rms_right"];
    
    for (level, name) in levels.iter().zip(names.iter()) {
        if level.is_nan() {
            return Err(anyhow::anyhow!("Audio level {} is NaN", name));
        }
        if level.is_infinite() {
            return Err(anyhow::anyhow!("Audio level {} is infinite", name));
        }
        if *level < 0.0 {
            return Err(anyhow::anyhow!("Audio level {} cannot be negative: {}", name, level));
        }
        if *level > 100.0 {
            return Err(anyhow::anyhow!("Audio level {} is too large: {} (maximum: 100.0)", name, level));
        }
    }
    
    // RMS should not exceed peak
    if rms_left > peak_left {
        return Err(anyhow::anyhow!("RMS left ({}) cannot exceed peak left ({})", rms_left, peak_left));
    }
    if rms_right > peak_right {
        return Err(anyhow::anyhow!("RMS right ({}) cannot exceed peak right ({})", rms_right, peak_right));
    }
    
    Ok(())
}

/// Security utilities for input sanitization
pub struct SecurityUtils;

impl SecurityUtils {
    /// Check if a string contains potentially dangerous patterns
    pub fn contains_dangerous_patterns(input: &str) -> bool {
        let dangerous_patterns = [
            "../", "..\\", "//", "\\\\", ";;", "&&", "||",
            "<script", "</script", "javascript:", "vbscript:",
            "onload=", "onerror=", "onclick=", "eval(",
            "exec(", "system(", "shell_exec(", "`",
        ];
        
        let input_lower = input.to_lowercase();
        dangerous_patterns.iter().any(|&pattern| input_lower.contains(pattern))
    }
    
    /// Sanitize a string for safe usage in device names
    pub fn sanitize_device_name(name: &str) -> String {
        name.chars()
            .filter(|&c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ' ' | '(' | ')'))
            .take(64) // Limit length
            .collect()
    }
    
    /// Validate that a number is within safe floating point bounds
    pub fn validate_safe_float(value: f32, name: &str) -> Result<()> {
        if value.is_nan() {
            return Err(anyhow::anyhow!("{} is NaN", name));
        }
        if value.is_infinite() {
            return Err(anyhow::anyhow!("{} is infinite", name));
        }
        if value.abs() > 1e6 {
            return Err(anyhow::anyhow!("{} is too large: {}", name, value));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_id_validation() {
        // Valid device IDs
        assert!(validate_device_id("output_blackhole").is_ok());
        assert!(validate_device_id("input_mic_01").is_ok());
        assert!(validate_device_id("device.with.dots").is_ok());
        assert!(validate_device_id("device:with:colons").is_ok());
        
        // Invalid device IDs
        assert!(validate_device_id("").is_err());
        assert!(validate_device_id("a").is_err());
        assert!(validate_device_id("_starts_with_underscore").is_err());
        assert!(validate_device_id("ends_with_underscore_").is_err());
        assert!(validate_device_id("has spaces").is_err());
        assert!(validate_device_id("has../path").is_err());
        assert!(validate_device_id("has;;semicolons").is_err());
    }

    #[test]
    fn test_config_validation() {
        let valid_config = MixerConfig {
            sample_rate: 48000,
            buffer_size: 512,
            channels: 2,
        };
        assert!(validate_config(&valid_config).is_ok());
        
        let invalid_rate = MixerConfig {
            sample_rate: 5000, // Too low
            buffer_size: 512,
            channels: 2,
        };
        assert!(validate_config(&invalid_rate).is_err());
    }

    #[test]
    fn test_security_utils() {
        assert!(SecurityUtils::contains_dangerous_patterns("../evil/path"));
        assert!(SecurityUtils::contains_dangerous_patterns("<script>alert('xss')</script>"));
        assert!(!SecurityUtils::contains_dangerous_patterns("normal_device_name"));
        
        let sanitized = SecurityUtils::sanitize_device_name("Device Name (With Spaces)");
        assert_eq!(sanitized, "Device Name (With Spaces)");
        
        let dangerous = SecurityUtils::sanitize_device_name("<script>evil</script>");
        assert!(!dangerous.contains("<script>"));
    }
}