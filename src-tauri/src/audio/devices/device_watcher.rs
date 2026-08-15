// Turns Core Audio hotplug notifications into device-list updates, health
// changes, and mixer teardown.
//
// The HAL callback in `property_listener` only says that something changed.
// Working out what changed - and reacting to it - happens here, off the HAL
// thread, where blocking and locking are safe.

use anyhow::Result;
use colored::Colorize;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::{Sender, UnboundedReceiver};
use tokio::sync::Mutex as AsyncMutex;
use tracing::{info, warn};

use super::property_listener::{DeviceEvent, DevicePropertyListener, LOG_TAG};
use super::AudioDeviceManager;
use crate::audio::mixer::stream_management::AudioCommand;
use crate::audio::types::AudioDeviceInfo;

pub const DEVICES_CHANGED_EVENT: &str = "audio-devices-changed";
pub const DEVICE_DISCONNECTED_EVENT: &str = "audio-device-disconnected";

/// One physical event reaches us as several notifications - an interface
/// arriving is a device-list change plus, if the system promotes it, two
/// default changes. Draining the burst keeps that to a single re-enumeration.
const COALESCE_WINDOW: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDisconnectedEvent {
    pub device_id: String,
    pub device_name: String,
    pub is_input: bool,
}

/// Owns the listener registrations and the task reacting to them. Dropping it
/// unregisters from the HAL and stops the task.
pub struct DeviceWatcher {
    task: Option<JoinHandle<()>>,
    _listener: DevicePropertyListener,
}

impl DeviceWatcher {
    pub fn start(
        app: AppHandle,
        device_manager: Arc<AsyncMutex<AudioDeviceManager>>,
        audio_command_tx: Sender<AudioCommand>,
    ) -> Result<Self> {
        let (listener, receiver) = DevicePropertyListener::start()?;

        let task = tauri::async_runtime::spawn(watch_loop(
            app,
            device_manager,
            audio_command_tx,
            receiver,
        ));

        Ok(Self {
            task: Some(task),
            _listener: listener,
        })
    }
}

impl Drop for DeviceWatcher {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            info!("{} Stopped watching devices", LOG_TAG.on_cyan().black());
        }
    }
}

async fn watch_loop(
    app: AppHandle,
    device_manager: Arc<AsyncMutex<AudioDeviceManager>>,
    audio_command_tx: Sender<AudioCommand>,
    mut receiver: UnboundedReceiver<DeviceEvent>,
) {
    let mut known = match enumerate(&device_manager).await {
        Ok(devices) => devices,
        Err(error) => {
            warn!(
                "{} Could not take an initial device snapshot: {}",
                LOG_TAG.on_cyan().black(),
                error
            );
            HashMap::new()
        }
    };

    while let Some(event) = receiver.recv().await {
        drain_burst(&mut receiver).await;

        let current = match enumerate(&device_manager).await {
            Ok(devices) => devices,
            Err(error) => {
                warn!(
                    "{} Re-enumeration failed after {:?}: {}",
                    LOG_TAG.on_cyan().black(),
                    event,
                    error
                );
                continue;
            }
        };

        handle_departures(&app, &device_manager, &audio_command_tx, &known, &current).await;
        emit_device_list(&app, &current);
        known = current;
    }

    info!(
        "{} Event stream closed, watcher exiting",
        LOG_TAG.on_cyan().black()
    );
}

/// Swallow the rest of a notification burst so one physical change causes one
/// pass, rather than one pass per property the HAL touched.
async fn drain_burst(receiver: &mut UnboundedReceiver<DeviceEvent>) {
    tokio::time::sleep(COALESCE_WINDOW).await;
    while receiver.try_recv().is_ok() {}
}

async fn enumerate(
    device_manager: &Arc<AsyncMutex<AudioDeviceManager>>,
) -> Result<HashMap<String, AudioDeviceInfo>> {
    let manager = device_manager.lock().await;
    let devices = manager.enumerate_devices().await?;

    Ok(devices
        .into_iter()
        .map(|device| (device.id.clone(), device))
        .collect())
}

/// Mark every device that vanished as disconnected and stop the mixer reading
/// from it.
///
/// The database configuration is deliberately left alone: it holds the
/// channel binding that reconnect needs to restore.
async fn handle_departures(
    app: &AppHandle,
    device_manager: &Arc<AsyncMutex<AudioDeviceManager>>,
    audio_command_tx: &Sender<AudioCommand>,
    known: &HashMap<String, AudioDeviceInfo>,
    current: &HashMap<String, AudioDeviceInfo>,
) {
    for (device_id, device) in known {
        if current.contains_key(device_id) {
            continue;
        }

        warn!(
            "{} {} ({}) disappeared",
            LOG_TAG.on_cyan().black(),
            device.name,
            device_id
        );

        {
            let manager = device_manager.lock().await;
            if let Err(error) = manager.check_device_health(device_id).await {
                warn!(
                    "{} Could not update health for {}: {}",
                    LOG_TAG.on_cyan().black(),
                    device_id,
                    error
                );
            }
        }

        tear_down_stream(audio_command_tx, device_id, device.is_input).await;

        emit(
            app,
            DEVICE_DISCONNECTED_EVENT,
            DeviceDisconnectedEvent {
                device_id: device_id.clone(),
                device_name: device.name.clone(),
                is_input: device.is_input,
            },
        );
    }
}

/// Input and output removals carry different response types, so they cannot
/// share one command value.
async fn tear_down_stream(
    audio_command_tx: &Sender<AudioCommand>,
    device_id: &str,
    is_input: bool,
) {
    let sent = if is_input {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let command = AudioCommand::RemoveInputStream {
            device_id: device_id.to_string(),
            response_tx,
        };
        let sent = audio_command_tx.send(command).await.is_ok();
        if sent {
            let _ = response_rx.await;
        }
        sent
    } else {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let command = AudioCommand::RemoveOutputStream {
            device_id: device_id.to_string(),
            response_tx,
        };
        let sent = audio_command_tx.send(command).await.is_ok();
        if sent {
            let _ = response_rx.await;
        }
        sent
    };

    if !sent {
        warn!(
            "{} Audio system unavailable, could not remove {}",
            LOG_TAG.on_cyan().black(),
            device_id
        );
    }
}

fn emit_device_list(app: &AppHandle, devices: &HashMap<String, AudioDeviceInfo>) {
    let mut list: Vec<AudioDeviceInfo> = devices.values().cloned().collect();
    list.sort_by(|a, b| a.id.cmp(&b.id));

    info!(
        "{} Publishing {} devices after change",
        LOG_TAG.on_cyan().black(),
        list.len()
    );

    emit(app, DEVICES_CHANGED_EVENT, list);
}

fn emit<P: Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(error) = app.emit(event, payload) {
        warn!(
            "{} Failed to emit {}: {}",
            LOG_TAG.on_cyan().black(),
            event,
            error
        );
    }
}
