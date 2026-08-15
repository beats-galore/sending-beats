use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tracing::info;

const DRIVER_NAME: &str = "SweetBeatsStudio.driver";
/// Must match DEVICE_NAME in src-driver/Makefile, which is what the HAL plugin
/// publishes to CoreAudio.
const DRIVER_DEVICE_NAME: &str = "Sweet Beats Audio";
const HAL_PLUGIN_DIR: &str = "/Library/Audio/Plug-Ins/HAL";
/// Restart coreaudiod so it rescans the HAL plug-in directory
///
/// `launchctl kickstart` has been observed to report success without actually
/// replacing the running daemon, which leaves a freshly copied driver unloaded,
/// so fall back to killing it and letting launchd bring it straight back up.
const RELOAD_COREAUDIOD_COMMAND: &str =
    "launchctl kickstart -kp system/com.apple.audio.coreaudiod || killall coreaudiod";

/// Time coreaudiod needs to restart and re-enumerate devices before the new
/// driver shows up in device enumeration
const COREAUDIOD_RESTART_DELAY: Duration = Duration::from_millis(1500);

const INSTALL_PROMPT: &str =
    "Sweet Beats Studio needs to install its virtual audio driver so system audio can be routed through the mixer.";

const RELOAD_PROMPT: &str =
    "Sweet Beats Studio needs to restart the macOS audio daemon to load its virtual audio driver.";

const UNINSTALL_PROMPT: &str = "Sweet Beats Studio needs to remove its virtual audio driver.";

/// Wrap a value in single quotes for safe interpolation into a /bin/sh command
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Escape a shell command so it can be embedded in an AppleScript string literal
fn applescript_quote(value: &str) -> String {
    value.replace('\\', r"\\").replace('"', "\\\"")
}

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
    ///
    /// The location differs between a bundled .app and `tauri dev`, which runs the
    /// bare binary with no surrounding bundle, so each candidate is probed in turn.
    fn get_bundled_driver_path() -> Result<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();

        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(contents_dir) = exe_path.parent().and_then(|p| p.parent()) {
                let resources = contents_dir.join("Resources");
                // Tauri rewrites the leading `..` of a bundled resource path to `_up_`,
                // so `../src-driver/build/X.driver` lands here
                candidates.push(
                    resources
                        .join("_up_")
                        .join("src-driver")
                        .join("build")
                        .join(DRIVER_NAME),
                );
                candidates.push(resources.join("driver").join(DRIVER_NAME));
                candidates.push(resources.join(DRIVER_NAME));
            }
        }

        #[cfg(debug_assertions)]
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("src-driver")
                .join("build")
                .join(DRIVER_NAME),
        );

        for candidate in &candidates {
            if candidate.exists() {
                info!(
                    "{} Located bundled driver at: {}",
                    "DRIVER_SOURCE".bright_blue(),
                    candidate.display()
                );
                return Ok(candidate.clone());
            }
        }

        Err(anyhow::anyhow!(
            "Bundled driver '{}' not found. Searched: {}",
            DRIVER_NAME,
            candidates
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }

    /// Run a shell command as root via the macOS authorization prompt
    ///
    /// `sudo` is unusable here because a GUI-launched app has no controlling
    /// terminal for it to prompt on, so it fails before doing any work.
    ///
    /// `prompt` is shown in the authorization dialog. macOS still attributes the
    /// request to osascript, since naming the app itself would require shipping a
    /// signed privileged helper.
    async fn run_elevated(script: String, prompt: &str, action: &'static str) -> Result<()> {
        let applescript = format!(
            "do shell script \"{}\" with prompt \"{}\" with administrator privileges",
            applescript_quote(&script),
            applescript_quote(prompt)
        );

        let output = tokio::task::spawn_blocking(move || {
            Command::new("osascript")
                .arg("-e")
                .arg(&applescript)
                .output()
        })
        .await
        .context("Privileged helper task failed to run")?
        .context("Failed to invoke osascript for privileged operation")?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);

        // osascript reports a user-cancelled authorization dialog as error -128
        if stderr.contains("-128") {
            return Err(anyhow::anyhow!(
                "Administrator authorization was cancelled, so we could not {}",
                action
            ));
        }

        Err(anyhow::anyhow!("Failed to {}: {}", action, stderr.trim()))
    }

    /// Install the virtual audio driver
    /// Prompts the user once for administrator authorization
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

        info!(
            "{} Copying driver from {} to {}",
            "DRIVER_COPY".bright_blue(),
            bundled_driver.display(),
            target_path.display()
        );

        let source = shell_quote(&bundled_driver.to_string_lossy());
        let target = shell_quote(&target_path.to_string_lossy());
        let plugin_dir = shell_quote(HAL_PLUGIN_DIR);

        // Every privileged step runs in one invocation so the user authenticates
        // once. coreaudiod refuses to load HAL plugins that are not root-owned.
        let script = format!(
            "mkdir -p {plugin_dir} && rm -rf {target} && cp -R {source} {target} \
             && chown -R root:wheel {target} && chmod -R 755 {target} \
             && {RELOAD_COREAUDIOD_COMMAND}"
        );

        Self::run_elevated(script, INSTALL_PROMPT, "install the virtual audio driver").await?;

        tokio::time::sleep(COREAUDIOD_RESTART_DELAY).await;

        Self::verify_installation()?;

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

        let target = shell_quote(
            &PathBuf::from(HAL_PLUGIN_DIR)
                .join(DRIVER_NAME)
                .to_string_lossy(),
        );

        let script = format!("rm -rf {target} && {RELOAD_COREAUDIOD_COMMAND}");

        Self::run_elevated(
            script,
            UNINSTALL_PROMPT,
            "uninstall the virtual audio driver",
        )
        .await?;

        tokio::time::sleep(COREAUDIOD_RESTART_DELAY).await;

        info!(
            "{} Virtual audio driver uninstalled successfully",
            "DRIVER_SUCCESS".bright_green()
        );

        Ok(())
    }

    /// Restart coreaudiod so it rescans the HAL plug-in directory
    ///
    /// Recovers the case where the driver bundle is on disk but the daemon was
    /// never restarted, so the device is never published.
    pub async fn reload_coreaudiod() -> Result<()> {
        info!(
            "{} Restarting coreaudiod to load the virtual driver...",
            "DRIVER_RELOAD".bright_cyan()
        );

        Self::run_elevated(
            RELOAD_COREAUDIOD_COMMAND.to_string(),
            RELOAD_PROMPT,
            "restart the macOS audio daemon",
        )
        .await?;

        tokio::time::sleep(COREAUDIOD_RESTART_DELAY).await;

        info!("{} coreaudiod restarted", "DRIVER_RELOADED".bright_green());

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
            .join("SweetBeatsStudio");

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
