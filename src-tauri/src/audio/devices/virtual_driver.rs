use anyhow::{Context, Result};
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{error, info, warn};

const DRIVER_NAME: &str = "SendinBeatsAudio.driver";
const DRIVER_DEVICE_NAME: &str = "Sendin Beats Audio";
const HAL_PLUGIN_DIR: &str = "/Library/Audio/Plug-Ins/HAL";

pub struct VirtualDriverManager;

impl VirtualDriverManager {
    /// Check if the virtual driver is installed
    pub fn is_installed() -> bool {
        let driver_path = PathBuf::from(HAL_PLUGIN_DIR).join(DRIVER_NAME);
        driver_path.exists()
    }

    /// Get the device UID by finding the device with our name
    pub async fn get_device_uid() -> Result<String> {
        use crate::audio::devices::AudioDeviceManager;

        let manager = AudioDeviceManager::new()?;
        let devices = manager.enumerate_devices().await?;

        for device in devices {
            if device.name == DRIVER_DEVICE_NAME && device.is_output {
                if let Some(uid) = device.uid {
                    return Ok(uid);
                }
            }
        }

        Err(anyhow::anyhow!(
            "Virtual audio device '{}' not found in system",
            DRIVER_DEVICE_NAME
        ))
    }

    /// Get the path to the bundled driver
    fn get_bundled_driver_path() -> Result<PathBuf> {
        // The driver is bundled in Resources/driver/
        let exe_path = std::env::current_exe()?;
        let app_dir = exe_path
            .parent()
            .and_then(|p| p.parent())
            .context("Failed to get app directory")?;

        let driver_path = app_dir.join("Resources").join("driver").join(DRIVER_NAME);

        if !driver_path.exists() {
            return Err(anyhow::anyhow!(
                "Bundled driver not found at: {}",
                driver_path.display()
            ));
        }

        Ok(driver_path)
    }

    /// Install the virtual audio driver
    /// This requires sudo privileges and will prompt the user
    pub async fn install() -> Result<()> {
        if Self::is_installed() {
            info!(
                "{} Virtual audio driver already installed",
                "DRIVER_INSTALLED".bright_green()
            );
            return Ok(());
        }

        info!(
            "{} Installing virtual audio driver...",
            "DRIVER_INSTALL".bright_cyan()
        );

        let bundled_driver = Self::get_bundled_driver_path()?;
        let target_path = PathBuf::from(HAL_PLUGIN_DIR).join(DRIVER_NAME);

        // Ensure HAL plugin directory exists
        let mkdir_status = Command::new("sudo")
            .args(&["mkdir", "-p", HAL_PLUGIN_DIR])
            .status()
            .context("Failed to create HAL plugin directory")?;

        if !mkdir_status.success() {
            return Err(anyhow::anyhow!(
                "Failed to create HAL plugin directory (requires sudo)"
            ));
        }

        // Copy driver bundle
        info!(
            "{} Copying driver from {} to {}",
            "DRIVER_COPY".bright_blue(),
            bundled_driver.display(),
            target_path.display()
        );

        let cp_status = Command::new("sudo")
            .args(&["cp", "-R", bundled_driver.to_str().unwrap(), HAL_PLUGIN_DIR])
            .status()
            .context("Failed to copy driver bundle")?;

        if !cp_status.success() {
            return Err(anyhow::anyhow!(
                "Failed to copy driver bundle (requires sudo)"
            ));
        }

        // Set proper permissions
        let chmod_status = Command::new("sudo")
            .args(&["chmod", "-R", "755", target_path.to_str().unwrap()])
            .status()
            .context("Failed to set driver permissions")?;

        if !chmod_status.success() {
            warn!(
                "{} Failed to set driver permissions, may cause issues",
                "DRIVER_WARN".bright_yellow()
            );
        }

        // Restart coreaudiod to load the driver
        Self::restart_coreaudiod()?;

        info!(
            "{} Virtual audio driver installed successfully",
            "DRIVER_SUCCESS".bright_green()
        );

        Ok(())
    }

    /// Uninstall the virtual audio driver
    pub async fn uninstall() -> Result<()> {
        if !Self::is_installed() {
            info!(
                "{} Virtual audio driver not installed",
                "DRIVER_NOT_INSTALLED".bright_yellow()
            );
            return Ok(());
        }

        info!(
            "{} Uninstalling virtual audio driver...",
            "DRIVER_UNINSTALL".bright_cyan()
        );

        let driver_path = PathBuf::from(HAL_PLUGIN_DIR).join(DRIVER_NAME);

        let rm_status = Command::new("sudo")
            .args(&["rm", "-rf", driver_path.to_str().unwrap()])
            .status()
            .context("Failed to remove driver bundle")?;

        if !rm_status.success() {
            return Err(anyhow::anyhow!(
                "Failed to remove driver bundle (requires sudo)"
            ));
        }

        // Restart coreaudiod to unload the driver
        Self::restart_coreaudiod()?;

        info!(
            "{} Virtual audio driver uninstalled successfully",
            "DRIVER_SUCCESS".bright_green()
        );

        Ok(())
    }

    /// Restart coreaudiod to reload drivers
    fn restart_coreaudiod() -> Result<()> {
        info!(
            "{} Restarting coreaudiod to reload drivers...",
            "DRIVER_RESTART".bright_blue()
        );

        // Use launchctl to restart coreaudiod
        let status = Command::new("sudo")
            .args(&[
                "launchctl",
                "kickstart",
                "-kp",
                "system/com.apple.audio.coreaudiod",
            ])
            .status()
            .context("Failed to restart coreaudiod")?;

        if !status.success() {
            error!(
                "{} Failed to restart coreaudiod, driver may not be loaded",
                "DRIVER_ERROR".bright_red()
            );
            return Err(anyhow::anyhow!("Failed to restart coreaudiod"));
        }

        // Give coreaudiod time to restart and enumerate devices
        std::thread::sleep(std::time::Duration::from_millis(500));

        info!(
            "{} coreaudiod restarted successfully",
            "DRIVER_RESTARTED".bright_green()
        );

        Ok(())
    }

    /// Get the name of the virtual driver device
    pub fn get_device_name() -> &'static str {
        DRIVER_DEVICE_NAME
    }

    /// Verify the driver is installed and functional
    pub fn verify_installation() -> Result<()> {
        if !Self::is_installed() {
            return Err(anyhow::anyhow!("Virtual audio driver is not installed"));
        }

        let driver_path = PathBuf::from(HAL_PLUGIN_DIR).join(DRIVER_NAME);
        let binary_path = driver_path
            .join("Contents")
            .join("MacOS")
            .join("SendinBeatsAudio");

        if !binary_path.exists() {
            return Err(anyhow::anyhow!(
                "Virtual audio driver binary not found at: {}",
                binary_path.display()
            ));
        }

        info!(
            "{} Virtual audio driver verified at: {}",
            "DRIVER_VERIFIED".bright_green(),
            driver_path.display()
        );

        Ok(())
    }
}
