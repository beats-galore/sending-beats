use anyhow::{Context, Result};
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::AppHandle;
use tracing::{info, warn};

const DRIVER_BUNDLE_NAME: &str = "SendinBeatsAudioDriver.bundle";
const HELPER_BINARY_NAME: &str = "sendin-beats-helper";
const DRIVER_DESTINATION: &str = "/Library/Audio/Plug-Ins/HAL/SendinBeatsAudioDriver.bundle";

pub struct DriverManager {
    app_handle: AppHandle,
}

impl DriverManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    pub fn is_driver_installed(&self) -> bool {
        Path::new(DRIVER_DESTINATION).exists()
    }

    pub fn get_helper_path(&self) -> Result<PathBuf> {
        let resource_path = self
            .app_handle
            .path()
            .resource_dir()
            .context("Failed to get resource directory")?;

        let helper_path = resource_path.join(HELPER_BINARY_NAME);

        if !helper_path.exists() {
            anyhow::bail!("Helper binary not found at: {:?}", helper_path);
        }

        Ok(helper_path)
    }

    pub fn get_driver_bundle_path(&self) -> Result<PathBuf> {
        let resource_path = self
            .app_handle
            .path()
            .resource_dir()
            .context("Failed to get resource directory")?;

        let driver_path = resource_path.join(DRIVER_BUNDLE_NAME);

        if !driver_path.exists() {
            anyhow::bail!("Driver bundle not found at: {:?}", driver_path);
        }

        Ok(driver_path)
    }

    pub fn install_driver(&self) -> Result<()> {
        if self.is_driver_installed() {
            info!("{}", "DRIVER_INSTALLED".green());
            return Ok(());
        }

        info!("{}", "DRIVER_INSTALL_START".blue());

        let helper_path = self.get_helper_path()?;
        let driver_path = self.get_driver_bundle_path()?;

        let osascript_command = format!(
            "do shell script \"'{}' install '{}'\" with administrator privileges",
            helper_path.display(),
            driver_path.display()
        );

        info!("{} {}", "DRIVER_INSTALL_EXEC".yellow(), osascript_command);

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&osascript_command)
            .output()
            .context("Failed to execute osascript for driver installation")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Driver installation failed: {}", stderr);
        }

        if !self.is_driver_installed() {
            anyhow::bail!("Driver installation completed but driver not found at destination");
        }

        info!("{}", "DRIVER_INSTALLED_SUCCESS".green());

        Ok(())
    }

    pub fn uninstall_driver(&self) -> Result<()> {
        if !self.is_driver_installed() {
            info!("{}", "DRIVER_NOT_INSTALLED".yellow());
            return Ok(());
        }

        info!("{}", "DRIVER_UNINSTALL_START".blue());

        let helper_path = self.get_helper_path()?;

        let osascript_command = format!(
            "do shell script \"'{}' uninstall\" with administrator privileges",
            helper_path.display()
        );

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&osascript_command)
            .output()
            .context("Failed to execute osascript for driver uninstallation")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Driver uninstallation failed: {}", stderr);
        }

        info!("{}", "DRIVER_UNINSTALLED_SUCCESS".green());

        Ok(())
    }

    pub fn start_helper_daemon(&self) -> Result<()> {
        let helper_path = self.get_helper_path()?;

        info!("{} {}", "HELPER_DAEMON_START".blue(), helper_path.display());

        let osascript_command = format!(
            "do shell script \"'{}' daemon &\" with administrator privileges",
            helper_path.display()
        );

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&osascript_command)
            .output()
            .context("Failed to start helper daemon")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("{} {}", "HELPER_DAEMON_FAILED".red(), stderr);
        } else {
            info!("{}", "HELPER_DAEMON_STARTED".green());
        }

        Ok(())
    }
}
