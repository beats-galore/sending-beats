use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{interval, Instant};
use tracing::{debug, info, warn};

use super::{AudioDeviceManager, DeviceStatus};
use crate::audio::mixer::stream_management::VirtualMixer;

/// Device monitoring service for automatic recovery and health tracking
///
/// This service runs in the background and:
/// - Monitors device health status
/// - Detects device disconnections and reconnections
/// - Attempts automatic stream recovery
/// - Provides fallback mechanisms for failed devices
#[derive(Debug)]
pub struct DeviceMonitor {
    /// Audio device manager reference
    device_manager: Arc<AsyncMutex<AudioDeviceManager>>,

    /// Virtual mixer reference
    mixer: std::sync::Weak<VirtualMixer>,

    /// Monitoring configuration
    config: DeviceMonitorConfig,

    /// Monitoring state
    is_running: Arc<std::sync::atomic::AtomicBool>,

    /// Statistics
    stats: Arc<tokio::sync::Mutex<DeviceMonitorStats>>,
}

#[derive(Debug, Clone)]
pub struct DeviceMonitorConfig {
    /// How often to check device health
    pub health_check_interval: Duration,

    /// How often to attempt recovery for failed devices
    pub recovery_check_interval: Duration,

    /// Maximum consecutive errors before considering device permanently failed
    pub max_consecutive_errors: u32,

    /// Delay before attempting recovery after device reconnection
    pub recovery_delay: Duration,

    /// Maximum number of recovery attempts per device
    pub max_recovery_attempts: u32,
}

impl Default for DeviceMonitorConfig {
    fn default() -> Self {
        Self {
            health_check_interval: Duration::from_secs(5),
            recovery_check_interval: Duration::from_secs(10),
            max_consecutive_errors: 3,
            recovery_delay: Duration::from_secs(2),
            max_recovery_attempts: 3,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceMonitorStats {
    #[serde(with = "instant_serde")]
    pub monitoring_started_at: Instant,
    pub health_checks_performed: u64,
    pub devices_detected_disconnected: u32,
    pub devices_detected_reconnected: u32,
    pub recovery_attempts_total: u32,
    pub recovery_attempts_successful: u32,
    pub recovery_attempts_failed: u32,
    #[serde(with = "instant_optional_serde")]
    pub last_health_check: Option<Instant>,
}

// Custom serializers for Instant
mod instant_serde {
    use serde::{Serialize, Serializer};
    use std::time::SystemTime;
    use tokio::time::Instant;

    pub fn serialize<S>(instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let elapsed = instant.elapsed();
        let system_time = SystemTime::now() - elapsed;
        let timestamp = system_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        timestamp.serialize(serializer)
    }
}

mod instant_optional_serde {
    use serde::{Serialize, Serializer};
    use std::time::SystemTime;
    use tokio::time::Instant;

    pub fn serialize<S>(instant_opt: &Option<Instant>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match instant_opt {
            Some(instant) => {
                let elapsed = instant.elapsed();
                let system_time = SystemTime::now() - elapsed;
                let timestamp = system_time
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                Some(timestamp).serialize(serializer)
            }
            None => None::<u64>.serialize(serializer),
        }
    }
}

impl Default for DeviceMonitorStats {
    fn default() -> Self {
        Self {
            monitoring_started_at: Instant::now(),
            health_checks_performed: 0,
            devices_detected_disconnected: 0,
            devices_detected_reconnected: 0,
            recovery_attempts_total: 0,
            recovery_attempts_successful: 0,
            recovery_attempts_failed: 0,
            last_health_check: None,
        }
    }
}

impl DeviceMonitor {
    /// Create a new device monitor
    pub fn new(
        device_manager: Arc<AsyncMutex<AudioDeviceManager>>,
        mixer: std::sync::Weak<VirtualMixer>,
        config: Option<DeviceMonitorConfig>,
    ) -> Self {
        Self {
            device_manager,
            mixer,
            config: config.unwrap_or_default(),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            stats: Arc::new(tokio::sync::Mutex::new(DeviceMonitorStats::default())),
        }
    }


        /// Start device monitoring service
        pub async fn start_monitoring(&self) -> Result<()> {
            if self
                .is_running
                .compare_exchange(
                    false,
                    true,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_err()
            {
                return Err(anyhow::anyhow!("Device monitor is already running"));
            }

            info!("🔍 Starting device monitoring service");

            // Clone references for the monitoring task
            let device_manager = self.device_manager.clone();
            let mixer_weak = self.mixer.clone();
            let config = self.config.clone();
            let is_running = self.is_running.clone();
            let stats = self.stats.clone();

            // Start the monitoring loop
            tokio::spawn(async move {
                Self::monitoring_loop(device_manager, mixer_weak, config, is_running, stats).await;
            });

            info!("✅ Device monitoring service started");
            Ok(())
        }

        /// Stop device monitoring service
        pub async fn stop_monitoring(&self) {
            self.is_running
                .store(false, std::sync::atomic::Ordering::SeqCst);
            info!("🛑 Device monitoring service stopped");
        }


    /// Get monitoring statistics
    pub async fn get_stats(&self) -> DeviceMonitorStats {
        self.stats.lock().await.clone()
    }

    /// Check if monitoring is active
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }



     /// Main monitoring loop
     async fn monitoring_loop(
        device_manager: Arc<AsyncMutex<AudioDeviceManager>>,
        mixer_weak: std::sync::Weak<VirtualMixer>,
        config: DeviceMonitorConfig,
        is_running: Arc<std::sync::atomic::AtomicBool>,
        stats: Arc<tokio::sync::Mutex<DeviceMonitorStats>>,
    ) {
        let mut health_check_interval = interval(config.health_check_interval);
        let mut recovery_check_interval = interval(config.recovery_check_interval);

        info!("🔄 Device monitoring loop started");

        while is_running.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::select! {
                _ = health_check_interval.tick() => {
                    if let Some(mixer) = mixer_weak.upgrade() {
                        Self::perform_health_check(&device_manager, &mixer, &config, &stats).await;
                    } else {
                        warn!("⚠️ Mixer reference lost, stopping device monitoring");
                        break;
                    }
                }

                _ = recovery_check_interval.tick() => {
                    if let Some(mixer) = mixer_weak.upgrade() {
                        Self::attempt_device_recovery(&device_manager, &mixer, &config, &stats).await;
                    } else {
                        warn!("⚠️ Mixer reference lost, stopping device monitoring");
                        break;
                    }
                }

                else => {
                    debug!("Device monitoring loop interrupted");
                    break;
                }
            }
        }

        info!("🛑 Device monitoring loop ended");
    }

    pub async fn stop_device_monitoring() -> Result<()> {
        if let Some(monitor) = DEVICE_MONITOR.get() {
            monitor.stop_monitoring().await;
            info!("✅ Device monitoring stopped");
        }
        Ok(())
    }


    /// Perform health check on all tracked devices
    async fn perform_health_check(
        device_manager: &Arc<AsyncMutex<AudioDeviceManager>>,
        mixer: &Arc<VirtualMixer>,
        config: &DeviceMonitorConfig,
        stats: &Arc<tokio::sync::Mutex<DeviceMonitorStats>>,
    ) {
        debug!("🔍 Performing device health check");

        // // Update stats
        // {
        //     let mut stats_guard = stats.lock().await;
        //     stats_guard.health_checks_performed += 1;
        //     stats_guard.last_health_check = Some(Instant::now());
        // }

        // // Get all device health statuses
        // let health_statuses = mixer.get_all_device_health_statuses().await;

        // for (device_id, health) in health_statuses {
        //     match health.status {
        //         DeviceStatus::Disconnected => {
        //             debug!(
        //                 "🔌 Device {} is disconnected, checking for reconnection",
        //                 device_id
        //             );

        //             // Check if device has reconnected
        //             match device_manager
        //                 .lock()
        //                 .await
        //                 .check_device_health(&device_id)
        //                 .await
        //             {
        //                 Ok(DeviceStatus::Connected) => {
        //                     info!("✅ Device {} has reconnected", device_id);

        //                     // Update stats
        //                     {
        //                         let mut stats_guard = stats.lock().await;
        //                         stats_guard.devices_detected_reconnected += 1;
        //                     }

        //                     // Mark for recovery attempt
        //                     device_manager
        //                         .lock()
        //                         .await
        //                         .report_device_error(
        //                             &device_id,
        //                             "Device reconnected - ready for recovery".to_string(),
        //                         )
        //                         .await;
        //                 }
        //                 Ok(DeviceStatus::Disconnected) => {
        //                     debug!("🔌 Device {} still disconnected", device_id);
        //                 }
        //                 Ok(DeviceStatus::Error(e)) => {
        //                     debug!("❌ Device {} has error: {}", device_id, e);
        //                 }
        //                 Err(e) => {
        //                     debug!("⚠️ Failed to check device {} health: {}", device_id, e);
        //                 }
        //             }
        //         }

        //         DeviceStatus::Error(ref error_msg) => {
        //             if health.consecutive_errors >= config.max_consecutive_errors {
        //                 warn!(
        //                     "❌ Device {} has persistent errors ({}): {}",
        //                     device_id, health.consecutive_errors, error_msg
        //                 );

        //                 // Update stats
        //                 {
        //                     let mut stats_guard = stats.lock().await;
        //                     stats_guard.devices_detected_disconnected += 1;
        //                 }
        //             } else {
        //                 debug!("⚠️ Device {} has error: {}", device_id, error_msg);
        //             }
        //         }

        //         DeviceStatus::Connected => {
        //             debug!("✅ Device {} is healthy", device_id);
        //         }
        //     }
        // }
    }

    /// Attempt recovery for devices that need it
    async fn attempt_device_recovery(
        device_manager: &Arc<AsyncMutex<AudioDeviceManager>>,
        mixer: &Arc<VirtualMixer>,
        config: &DeviceMonitorConfig,
        stats: &Arc<tokio::sync::Mutex<DeviceMonitorStats>>,
    ) {
        debug!("🔧 Checking for devices needing recovery");

        // let health_statuses = mixer.get_all_device_health_statuses().await;

        // for (device_id, health) in health_statuses {
        //     // Check if device needs recovery
        //     let needs_recovery = match health.status {
        //         DeviceStatus::Disconnected => {
        //             // Check if device has reconnected recently
        //             device_manager
        //                 .lock()
        //                 .await
        //                 .check_device_health(&device_id)
        //                 .await
        //                 .map(|status| matches!(status, DeviceStatus::Connected))
        //                 .unwrap_or(false)
        //         }
        //         DeviceStatus::Error(_)
        //             if health.consecutive_errors >= config.max_consecutive_errors =>
        //         {
        //             // Try recovery for persistently failed devices
        //             true
        //         }
        //         _ => false,
        //     };

        //     if needs_recovery {
        //         info!("🔧 Attempting recovery for device: {}", device_id);

        //         // Update stats
        //         {
        //             let mut stats_guard = stats.lock().await;
        //             stats_guard.recovery_attempts_total += 1;
        //         }

        //         // Wait for device to stabilize
        //         tokio::time::sleep(config.recovery_delay).await;

        //         // Attempt stream recovery
        //         match Self::recover_device_stream(mixer, &device_id).await {
        //             Ok(()) => {
        //                 info!("✅ Successfully recovered device: {}", device_id);

        //                 // Update stats
        //                 {
        //                     let mut stats_guard = stats.lock().await;
        //                     stats_guard.recovery_attempts_successful += 1;
        //                 }

        //                 // Reset device error count
        //                 device_manager
        //                     .lock()
        //                     .await
        //                     .report_device_error(
        //                         &device_id,
        //                         "Device recovered successfully".to_string(),
        //                     )
        //                     .await;
        //             }
        //             Err(e) => {
        //                 warn!("❌ Failed to recover device {}: {}", device_id, e);

        //                 // Update stats
        //                 {
        //                     let mut stats_guard = stats.lock().await;
        //                     stats_guard.recovery_attempts_failed += 1;
        //                 }
        //             }
        //         }
        //     }
        // }
    }

    /// Recover a specific device stream
    async fn recover_device_stream(mixer: &Arc<VirtualMixer>, device_id: &str) -> Result<()> {
        Ok(())
        // debug!("🔄 Recovering stream for device: {}", device_id);

        // // **STREAMLINED ARCHITECTURE**: Device stream recovery now handled by IsolatedAudioManager
        // debug!("⚠️ DEVICE RECOVERY: Stream recovery now managed by IsolatedAudioManager, not VirtualMixer");

        // // Wait a moment for cleanup
        // tokio::time::sleep(Duration::from_millis(500)).await;

        // // Attempt recreation using safe method
        // match mixer.add_input_stream_safe(device_id).await {
        //     Ok(()) => {
        //         info!("✅ Successfully recovered stream for device: {}", device_id);
        //         Ok(())
        //     }
        //     Err(e) => {
        //         warn!(
        //             "❌ Failed to recover stream for device {}: {}",
        //             device_id, e
        //         );
        //         Err(e)
        //     }
        // }
    }
}

/// Global device monitor instance
static DEVICE_MONITOR: tokio::sync::OnceCell<Arc<DeviceMonitor>> =
    tokio::sync::OnceCell::const_new();


/// Get the global device monitor
pub async fn get_device_monitor() -> Option<Arc<DeviceMonitor>> {
    DEVICE_MONITOR.get().cloned()
}


/// Get device monitoring statistics
pub async fn get_device_monitoring_stats() -> Option<DeviceMonitorStats> {
    if let Some(monitor) = DEVICE_MONITOR.get() {
        Some(monitor.get_stats().await)
    } else {
        None
    }
}
