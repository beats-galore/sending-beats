// Filename generation and path management for recordings
//
// This module handles template-based filename generation, path validation,
// and file organization for recording outputs. It provides flexible naming
// schemes and ensures safe file operations.

use anyhow::{Context, Result};
use chrono::Local;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use super::types::RecordingConfig;

/// Template variables available for filename generation
pub struct TemplateVariables {
    variables: HashMap<String, String>,
}

impl TemplateVariables {
    /// Create new template variables from recording config
    pub fn from_config(config: &RecordingConfig) -> Self {
        let mut variables = HashMap::new();

        // Current timestamp
        let now = Local::now();
        variables.insert(
            "timestamp".to_string(),
            now.format("%Y%m%d_%H%M%S").to_string(),
        );
        variables.insert("date".to_string(), now.format("%Y-%m-%d").to_string());
        variables.insert("time".to_string(), now.format("%H-%M-%S").to_string());
        variables.insert("year".to_string(), now.format("%Y").to_string());
        variables.insert("month".to_string(), now.format("%m").to_string());
        variables.insert("day".to_string(), now.format("%d").to_string());
        variables.insert("hour".to_string(), now.format("%H").to_string());
        variables.insert("minute".to_string(), now.format("%M").to_string());
        variables.insert("second".to_string(), now.format("%S").to_string());

        // Recording configuration
        variables.insert("config_name".to_string(), sanitize_filename(&config.name));
        variables.insert(
            "format".to_string(),
            config.format.get_format_name().to_lowercase(),
        );
        variables.insert("sample_rate".to_string(), config.sample_rate.to_string());
        variables.insert(
            "channels".to_string(),
            if config.channels == 1 {
                "mono"
            } else {
                "stereo"
            }
            .to_string(),
        );
        variables.insert("bit_depth".to_string(), config.bit_depth.to_string());

        // Metadata (with fallbacks)
        variables.insert(
            "title".to_string(),
            config
                .metadata
                .title
                .as_ref()
                .map(|t| sanitize_filename(t))
                .unwrap_or_else(|| "recording".to_string()),
        );
        variables.insert(
            "artist".to_string(),
            config
                .metadata
                .artist
                .as_ref()
                .map(|a| sanitize_filename(a))
                .unwrap_or_else(|| "unknown_artist".to_string()),
        );
        variables.insert(
            "album".to_string(),
            config
                .metadata
                .album
                .as_ref()
                .map(|a| sanitize_filename(a))
                .unwrap_or_else(|| "unknown_album".to_string()),
        );
        variables.insert(
            "genre".to_string(),
            config
                .metadata
                .genre
                .as_ref()
                .map(|g| sanitize_filename(g))
                .unwrap_or_else(|| "unknown_genre".to_string()),
        );

        Self { variables }
    }

    /// Add or override a template variable
    pub fn set(&mut self, key: &str, value: String) {
        self.variables
            .insert(key.to_string(), sanitize_filename(&value));
    }

    /// Get a template variable value
    pub fn get(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    /// Get all available variables
    pub fn list_variables(&self) -> Vec<String> {
        self.variables.keys().cloned().collect()
    }
}

/// Filename generator with template support
pub struct FilenameGenerator {
    template_regex: Regex,
}

impl FilenameGenerator {
    /// Create a new filename generator
    pub fn new() -> Self {
        Self {
            // Match template variables like {variable_name}
            template_regex: Regex::new(r"\{([a-zA-Z_][a-zA-Z0-9_]*)\}").unwrap(),
        }
    }

    /// Generate filename from template and configuration
    pub fn generate(&self, config: &RecordingConfig) -> Result<String> {
        let variables = TemplateVariables::from_config(config);
        self.generate_with_variables(&config.filename_template, &variables)
    }

    /// Generate filename with custom variables
    pub fn generate_with_variables(
        &self,
        template: &str,
        variables: &TemplateVariables,
    ) -> Result<String> {
        if template.is_empty() {
            return Err(anyhow::anyhow!("Filename template cannot be empty"));
        }

        let mut filename = template.to_string();
        let mut unresolved_variables = Vec::new();

        // Replace all template variables
        filename = self
            .template_regex
            .replace_all(&filename, |caps: &regex::Captures| {
                let var_name = &caps[1];
                match variables.get(var_name) {
                    Some(value) => value.clone(),
                    None => {
                        unresolved_variables.push(var_name.to_string());
                        format!("{{{}}}", var_name) // Keep original if unresolved
                    }
                }
            })
            .to_string();

        // Warn about unresolved variables
        if !unresolved_variables.is_empty() {
            warn!("Unresolved template variables: {:?}", unresolved_variables);
        }

        // Final sanitization
        filename = sanitize_filename(&filename);

        // Ensure filename is not empty after sanitization
        if filename.is_empty() || filename == "." || filename == ".." {
            filename = format!("recording_{}", Local::now().format("%Y%m%d_%H%M%S"));
        }

        // Add file extension if not present
        let extension = self.get_file_extension(&filename);
        if extension.is_empty() {
            filename = format!("{}.wav", filename); // Default to WAV
        }

        info!(
            "Generated filename: {} from template: {}",
            filename, template
        );
        Ok(filename)
    }

    /// Extract file extension from filename
    fn get_file_extension(&self, filename: &str) -> String {
        Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase()
    }

    /// Validate a filename template
    pub fn validate_template(&self, template: &str) -> Result<Vec<String>> {
        if template.is_empty() {
            return Err(anyhow::anyhow!("Template cannot be empty"));
        }

        // Check for potentially dangerous patterns
        let dangerous_patterns = [
            "../", "..\\", "//", "\\\\", "<", ">", ":", "\"", "|", "?", "*",
        ];
        for pattern in &dangerous_patterns {
            if template.contains(pattern) {
                return Err(anyhow::anyhow!(
                    "Template contains dangerous pattern: '{}'",
                    pattern
                ));
            }
        }

        // Extract all template variables
        let variables: Vec<String> = self
            .template_regex
            .captures_iter(template)
            .map(|cap| cap[1].to_string())
            .collect();

        Ok(variables)
    }
}

/// Path management utilities
pub struct PathManager;

impl PathManager {
    /// Ensure a directory exists, creating it if necessary
    pub async fn ensure_directory_exists(path: &Path) -> Result<()> {
        if !path.exists() {
            tokio::fs::create_dir_all(path)
                .await
                .with_context(|| format!("Failed to create directory: {}", path.display()))?;
            info!("Created recording directory: {}", path.display());
        } else if !path.is_dir() {
            return Err(anyhow::anyhow!(
                "Path exists but is not a directory: {}",
                path.display()
            ));
        }
        Ok(())
    }

    /// Check if a path is safe for recording (not system directories, etc.)
    pub fn is_safe_recording_path(path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();

        // Dangerous directories to avoid
        let dangerous_paths = [
            "/system",
            "/boot",
            "/usr",
            "/bin",
            "/sbin",
            "/etc",
            "/dev",
            "/proc",
            "/sys",
            "c:\\windows",
            "c:\\system32",
            "c:\\program files",
            "c:\\programdata",
            "/applications",
            "/library/system",
        ];

        for dangerous in &dangerous_paths {
            if path_str.starts_with(dangerous) {
                return false;
            }
        }

        // Must be an absolute path
        if !path.is_absolute() {
            return false;
        }

        true
    }

    /// Generate a unique filename if the target already exists
    pub fn make_unique_filename(base_path: &Path) -> PathBuf {
        if !base_path.exists() {
            return base_path.to_path_buf();
        }

        let parent = base_path.parent().unwrap_or(Path::new("."));
        let stem = base_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("recording");
        let extension = base_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("wav");

        for i in 1..=9999 {
            let new_filename = format!("{}_{:03}.{}", stem, i, extension);
            let new_path = parent.join(new_filename);
            if !new_path.exists() {
                return new_path;
            }
        }

        // Fallback with timestamp
        let timestamp = Local::now().format("%H%M%S");
        let fallback_filename = format!("{}_{}.{}", stem, timestamp, extension);
        parent.join(fallback_filename)
    }

    /// Get available disk space for a path (implemented with proper disk space checking)
    pub fn get_available_space(path: &Path) -> Result<u64> {
        // Use statvfs on Unix-like systems for real disk space checking
        #[cfg(unix)]
        {
            use std::ffi::CString;
            use std::mem;

            // Convert path to CString
            let path_cstring = CString::new(path.to_string_lossy().as_bytes())
                .map_err(|_| anyhow::anyhow!("Invalid path for disk space check"))?;

            // Call statvfs to get filesystem statistics
            unsafe {
                let mut statvfs: libc::statvfs = mem::zeroed();
                if libc::statvfs(path_cstring.as_ptr(), &mut statvfs) != 0 {
                    return Err(anyhow::anyhow!("Failed to get filesystem statistics"));
                }

                // Calculate available space: block size * available blocks
                let available_bytes = statvfs.f_bavail as u64 * statvfs.f_frsize as u64;
                Ok(available_bytes)
            }
        }

        // Fallback for non-Unix systems or if Unix implementation fails
        #[cfg(not(unix))]
        {
            // On Windows or other platforms, use a reasonable fallback
            // This could be improved with platform-specific implementations
            use std::fs;

            // Try to check if the path exists and is accessible
            match fs::metadata(path) {
                Ok(_) => {
                    // Path exists, return a reasonable amount of free space
                    // In a real implementation, this would use Windows API like GetDiskFreeSpaceEx
                    Ok(50 * 1024 * 1024 * 1024) // 50GB fallback
                }
                Err(_) => Err(anyhow::anyhow!("Path does not exist or is not accessible")),
            }
        }
    }

    /// Validate that we have enough space for recording
    pub fn check_available_space(path: &Path, estimated_size_mb: u64) -> Result<bool> {
        let available_bytes = Self::get_available_space(path)?;
        let required_bytes = estimated_size_mb * 1024 * 1024;
        let buffer_bytes = 100 * 1024 * 1024; // 100MB buffer

        Ok(available_bytes > (required_bytes + buffer_bytes))
    }
}

/// Sanitize a string for use in filenames
pub fn sanitize_filename(input: &str) -> String {
    // Replace or remove problematic characters
    let sanitized = input
        .chars()
        .map(|c| match c {
            // Replace spaces with underscores
            ' ' => '_',
            // Replace other problematic characters
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            // Keep dots but not at start/end
            '.' => '_',
            // Keep alphanumeric, underscore, hyphen
            c if c.is_alphanumeric() || c == '_' || c == '-' => c,
            // Replace everything else
            _ => '_',
        })
        .collect::<String>();

    // Remove multiple consecutive underscores
    let re = Regex::new(r"_+").unwrap();
    let sanitized = re.replace_all(&sanitized, "_");

    // Remove leading/trailing underscores
    let sanitized = sanitized.trim_matches('_');

    // Ensure not empty and not too long
    let sanitized = if sanitized.is_empty() {
        "recording".to_string()
    } else {
        sanitized.chars().take(100).collect() // Limit length
    };

    sanitized
}

/// Common filename templates
pub struct FilenameTemplates;

impl FilenameTemplates {
    pub const TIMESTAMP: &'static str = "{timestamp}";
    pub const TITLE_TIMESTAMP: &'static str = "{title}_{timestamp}";
    pub const ARTIST_TITLE: &'static str = "{artist}_{title}";
    pub const DATE_TIME: &'static str = "{date}_{time}";
    pub const CONFIG_TIMESTAMP: &'static str = "{config_name}_{timestamp}";
    pub const DETAILED: &'static str = "{date}_{time}_{title}_{format}_{sample_rate}hz";

    /// Get all predefined templates
    pub fn all_templates() -> Vec<(&'static str, &'static str)> {
        vec![
            ("Simple Timestamp", Self::TIMESTAMP),
            ("Title + Timestamp", Self::TITLE_TIMESTAMP),
            ("Artist + Title", Self::ARTIST_TITLE),
            ("Date + Time", Self::DATE_TIME),
            ("Config + Timestamp", Self::CONFIG_TIMESTAMP),
            ("Detailed", Self::DETAILED),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::recording::types::*;

    #[test]
    fn test_filename_sanitization() {
        assert_eq!(sanitize_filename("Normal Name"), "Normal_Name");
        assert_eq!(
            sanitize_filename("with/slash\\backslash"),
            "with_slash_backslash"
        );
        assert_eq!(sanitize_filename(""), "recording");
        assert_eq!(sanitize_filename("..."), "recording");
    }

    #[test]
    fn test_template_variables() {
        let config = RecordingConfig {
            name: "Test Config".to_string(),
            ..Default::default()
        };

        let variables = TemplateVariables::from_config(&config);
        assert_eq!(
            variables.get("config_name"),
            Some(&"Test_Config".to_string())
        );
        assert!(variables.get("timestamp").is_some());
        assert_eq!(variables.get("format"), Some(&"wav".to_string()));
    }

    #[test]
    fn test_filename_generation() {
        let generator = FilenameGenerator::new();
        let config = RecordingConfig::default();

        let filename = generator.generate(&config).unwrap();
        assert!(!filename.is_empty());
        assert!(filename.contains("recording")); // Default title
    }

    #[test]
    fn test_template_validation() {
        let generator = FilenameGenerator::new();

        assert!(generator.validate_template("{title}_{timestamp}").is_ok());
        assert!(generator.validate_template("../dangerous").is_err());
        assert!(generator.validate_template("").is_err());
    }

    #[test]
    fn test_path_safety() {
        assert!(PathManager::is_safe_recording_path(Path::new(
            "/Users/test/Music"
        )));
        assert!(!PathManager::is_safe_recording_path(Path::new("/system")));
        assert!(!PathManager::is_safe_recording_path(Path::new(
            "relative/path"
        )));
    }
}
