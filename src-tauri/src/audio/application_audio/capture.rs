use anyhow::{Context, Result};
use colored::Colorize;
use coreaudio::audio_unit::audio_format::LinearPcmFlags;
use coreaudio::audio_unit::{AudioUnit, Element, SampleFormat, Scope, StreamFormat};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::AppHandle;
use tracing::{info, warn};

use super::driver_manager::DriverManager;
use super::ipc_client::IPCClient;
use super::pid_manager::PIDManager;

const DEVICE_NAME: &str = "Sendin Beats Virtual Audio";
const SAMPLE_RATE: f64 = 48000.0;
const CHANNELS: u32 = 16;

pub struct ApplicationAudioCapture {
    driver_manager: DriverManager,
    ipc_client: IPCClient,
    pid_manager: Arc<PIDManager>,
    audio_unit: Option<AudioUnit>,
    is_running: Arc<AtomicBool>,
}

impl ApplicationAudioCapture {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            driver_manager: DriverManager::new(app_handle),
            ipc_client: IPCClient::new(),
            pid_manager: Arc::new(PIDManager::new()),
            audio_unit: None,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        info!("{}", "APP_AUDIO_INIT".blue());

        if !self.driver_manager.is_driver_installed() {
            info!("{}", "DRIVER_INSTALL_REQUIRED".yellow());
            self.driver_manager.install_driver()?;
        }

        if !self.ipc_client.is_helper_running() {
            info!("{}", "HELPER_START_REQUIRED".yellow());
            self.driver_manager.start_helper_daemon()?;
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            if !self.ipc_client.is_helper_running() {
                anyhow::bail!("Helper daemon failed to start");
            }
        }

        info!("{}", "APP_AUDIO_INIT_SUCCESS".green());

        Ok(())
    }

    pub fn capture_application(&self, pid: i32) -> Result<i32> {
        let channel = self.pid_manager.allocate_channel(pid)?;

        self.ipc_client
            .map_pid_to_channel(pid, channel)
            .context("Failed to map PID to channel via IPC")?;

        let app_name = self.pid_manager.get_process_name(pid).unwrap_or_else(|_| format!("PID {}", pid));

        info!("{} {} (PID {}) -> channel {}", "APP_CAPTURE_START".green(), app_name, pid, channel);

        Ok(channel)
    }

    pub fn stop_capturing_application(&self, pid: i32) -> Result<()> {
        self.ipc_client
            .unmap_pid(pid)
            .context("Failed to unmap PID via IPC")?;

        self.pid_manager.free_channel(pid)?;

        let app_name = self.pid_manager.get_process_name(pid).unwrap_or_else(|_| format!("PID {}", pid));

        info!("{} {} (PID {})", "APP_CAPTURE_STOP".green(), app_name, pid);

        Ok(())
    }

    pub fn list_applications(&self) -> Result<Vec<super::pid_manager::ApplicationInfo>> {
        self.pid_manager.list_applications()
    }

    pub fn get_pid_manager(&self) -> Arc<PIDManager> {
        Arc::clone(&self.pid_manager)
    }

    pub fn start_audio_unit(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            warn!("{}", "AUDIO_UNIT_ALREADY_RUNNING".yellow());
            return Ok(());
        }

        info!("{}", "AUDIO_UNIT_START".blue());

        let mut audio_unit = AudioUnit::new_input_output()?;

        let stream_format = StreamFormat {
            sample_rate: SAMPLE_RATE,
            sample_format: SampleFormat::F32,
            flags: LinearPcmFlags::IS_FLOAT | LinearPcmFlags::IS_PACKED,
            channels: CHANNELS,
        };

        audio_unit.set_property(
            coreaudio::audio_unit::Property::StreamFormat(stream_format),
            Scope::Input,
            Element::Input,
        )?;

        let is_running = Arc::clone(&self.is_running);

        audio_unit.set_input_callback(move |args| {
            if !is_running.load(Ordering::SeqCst) {
                return Ok(());
            }

            Ok(())
        })?;

        audio_unit.start()?;

        self.audio_unit = Some(audio_unit);
        self.is_running.store(true, Ordering::SeqCst);

        info!("{}", "AUDIO_UNIT_STARTED".green());

        Ok(())
    }

    pub fn stop_audio_unit(&mut self) -> Result<()> {
        if !self.is_running.load(Ordering::SeqCst) {
            return Ok(());
        }

        info!("{}", "AUDIO_UNIT_STOP".blue());

        self.is_running.store(false, Ordering::SeqCst);

        if let Some(mut audio_unit) = self.audio_unit.take() {
            audio_unit.stop()?;
        }

        info!("{}", "AUDIO_UNIT_STOPPED".green());

        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
}

impl Drop for ApplicationAudioCapture {
    fn drop(&mut self) {
        let _ = self.stop_audio_unit();
        self.pid_manager.clear_all();
    }
}
