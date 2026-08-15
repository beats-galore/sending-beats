// Turns Core Audio hotplug notifications into device-list updates, health
// changes, and mixer teardown.
//
// The HAL callback in `property_listener` only says that something changed.
// Working out what changed - and reacting to it - happens here, off the HAL
// thread, where blocking and locking are safe.

use anyhow::Result;
use colored::Colorize;
use coreaudio_sys::AudioObjectID;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc::{Sender, UnboundedReceiver};
use tokio::sync::Mutex as AsyncMutex;
use tracing::{info, warn};

use super::property_listener::{DeviceEvent, DevicePropertyListener, LOG_TAG};
use super::AudioDeviceManager;
use crate::audio::mixer::stream_management::AudioCommand;
use crate::audio::types::AudioDeviceInfo;
use crate::commands::device_attachment::{attach_input_device, attach_output_device};
use crate::AudioState;

pub const DEVICES_CHANGED_EVENT: &str = "audio-devices-changed";
pub const DEVICE_DISCONNECTED_EVENT: &str = "audio-device-disconnected";
pub const DEVICE_RECONNECTED_EVENT: &str = "audio-device-reconnected";

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceReconnectedEvent {
    pub device_id: String,
    pub device_name: String,
    pub channel_number: i32,
    /// False when the device came back but its stream could not be rebuilt.
    pub restored: bool,
}

/// Owns the task reacting to hotplug events. Aborting it drops the listener the
/// task holds, which unregisters every HAL registration.
pub struct DeviceWatcher {
    task: Option<JoinHandle<()>>,
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
            listener,
            receiver,
        ));

        Ok(Self { task: Some(task) })
    }
}

/// Restore every device that just came back and still has a saved
/// configuration.
///
/// Arrivals come out of Core Audio enumeration, so they are always hardware
/// devices - application sources are keyed `app-<bundle>` and never appear
/// here, which is why inputs always reattach as non-application audio.
async fn handle_arrivals(
    app: &AppHandle,
    known: &HashMap<String, AudioDeviceInfo>,
    current: &HashMap<String, AudioDeviceInfo>,
) {
    let arrivals: Vec<&AudioDeviceInfo> = current
        .values()
        .filter(|device| !known.contains_key(&device.id))
        .collect();

    if arrivals.is_empty() {
        return;
    }

    let Some(state) = app.try_state::<AudioState>() else {
        return;
    };

    for device in arrivals {
        let Some(configuration) = saved_configuration(&state, &device.id).await else {
            continue;
        };

        info!(
            "{} {} came back, restoring",
            LOG_TAG.on_cyan().black(),
            device.name
        );

        let outcome = if configuration.is_input {
            attach_input_device(
                &state,
                &device.id,
                false,
                Some(configuration.channel_number),
            )
            .await
            .map(|_| ())
        } else {
            attach_output_device(&state, &device.id).await
        };

        let restored = match outcome {
            Ok(()) => true,
            Err(error) => {
                warn!(
                    "{} Could not restore {}: {}",
                    LOG_TAG.on_cyan().black(),
                    device.id,
                    error
                );
                false
            }
        };

        emit(
            app,
            DEVICE_RECONNECTED_EVENT,
            DeviceReconnectedEvent {
                device_id: device.id.clone(),
                device_name: device.name.clone(),
                channel_number: configuration.channel_number,
                restored,
            },
        );
    }
}

/// Every device identifier the user has patched to something, in one query
/// rather than one per device on the machine.
async fn patched_device_identifiers(state: &AudioState) -> HashSet<String> {
    let found = crate::entities::configured_audio_device::Entity::find()
        .all(state.database.sea_orm())
        .await;

    match found {
        Ok(configurations) => configurations
            .into_iter()
            .map(|configuration| configuration.device_identifier)
            .collect(),
        Err(error) => {
            warn!(
                "{} Could not list saved device configurations: {}",
                LOG_TAG.on_cyan().black(),
                error
            );
            HashSet::new()
        }
    }
}

/// The saved binding for a device, if the user ever patched one.
///
/// Departure deliberately leaves this row in place, so its presence is what
/// separates a device worth restoring from one that merely showed up.
async fn saved_configuration(
    state: &AudioState,
    device_id: &str,
) -> Option<crate::entities::configured_audio_device::Model> {
    let found = crate::entities::configured_audio_device::Entity::find()
        .filter(crate::entities::configured_audio_device::Column::DeviceIdentifier.eq(device_id))
        .one(state.database.sea_orm())
        .await;

    match found {
        Ok(configuration) => configuration,
        Err(error) => {
            warn!(
                "{} Could not look up saved configuration for {}: {}",
                LOG_TAG.on_cyan().black(),
                device_id,
                error
            );
            None
        }
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
    mut listener: DevicePropertyListener,
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

    // Device id to CoreAudio object id, for everything currently registered for
    // individual death notifications.
    let mut watched: HashMap<String, AudioObjectID> = HashMap::new();
    sync_watched_devices(&app, &device_manager, &mut listener, &known, &mut watched).await;

    while let Some(event) = receiver.recv().await {
        let died = drain_burst(&mut receiver, event).await;

        let mut current = match enumerate(&device_manager).await {
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

        // A device can die while CoreAudio still lists it - an interface losing
        // power rather than being unplugged - so enumeration alone would call
        // it present. Drop it from the set to make the departure path fire.
        for device_id in dead_device_ids(&watched, &died) {
            if current.remove(&device_id).is_some() {
                warn!(
                    "{} {} stopped responding while still listed",
                    LOG_TAG.on_cyan().black(),
                    device_id
                );
            }
        }

        handle_departures(&app, &device_manager, &audio_command_tx, &known, &current).await;

        // The list goes out before recovery is attempted so the arrival shows
        // up immediately, and so a failed restore - reported afterwards - is
        // not overwritten by a list that says the device is present and fine.
        emit_device_list(&app, &current);
        handle_arrivals(&app, &known, &current).await;

        sync_watched_devices(&app, &device_manager, &mut listener, &current, &mut watched).await;
        known = current;
    }

    info!(
        "{} Event stream closed, watcher exiting",
        LOG_TAG.on_cyan().black()
    );
}

/// Swallow the rest of a notification burst so one physical change causes one
/// pass, rather than one pass per property the HAL touched, and report which
/// devices the burst said had died.
async fn drain_burst(
    receiver: &mut UnboundedReceiver<DeviceEvent>,
    first: DeviceEvent,
) -> HashSet<AudioObjectID> {
    let mut died = HashSet::new();
    let mut record = |event: DeviceEvent| {
        if let DeviceEvent::DeviceDied(object_id) = event {
            died.insert(object_id);
        }
    };

    record(first);
    tokio::time::sleep(COALESCE_WINDOW).await;
    while let Ok(event) = receiver.try_recv() {
        record(event);
    }

    died
}

/// One physical device is listed twice when it has both directions, so a single
/// dead object id can account for more than one device id.
fn dead_device_ids(
    watched: &HashMap<String, AudioObjectID>,
    died: &HashSet<AudioObjectID>,
) -> Vec<String> {
    watched
        .iter()
        .filter(|(_, object_id)| died.contains(object_id))
        .map(|(device_id, _)| device_id.clone())
        .collect()
}

/// Keep individual death notifications registered for exactly the devices that
/// are present and patched to something.
///
/// An unpatched device dying is not interesting - it is not carrying audio, and
/// its removal from the device list is caught by the system-wide listener.
async fn sync_watched_devices(
    app: &AppHandle,
    device_manager: &Arc<AsyncMutex<AudioDeviceManager>>,
    listener: &mut DevicePropertyListener,
    current: &HashMap<String, AudioDeviceInfo>,
    watched: &mut HashMap<String, AudioObjectID>,
) {
    let Some(state) = app.try_state::<AudioState>() else {
        return;
    };

    let patched = patched_device_identifiers(&state).await;

    let mut desired: HashMap<String, AudioObjectID> = HashMap::new();
    {
        let manager = device_manager.lock().await;
        for device in current.values() {
            let Some(uid) = device.uid.as_deref() else {
                continue;
            };
            if !patched.contains(&device.id) {
                continue;
            }

            match manager.coreaudio().translate_uid_to_device(uid) {
                Ok(object_id) => {
                    desired.insert(device.id.clone(), object_id);
                }
                Err(error) => {
                    warn!(
                        "{} No CoreAudio object for {} ({}): {}",
                        LOG_TAG.on_cyan().black(),
                        device.id,
                        uid,
                        error
                    );
                }
            }
        }
    }

    for (device_id, object_id) in watched.iter() {
        if !desired.contains_key(device_id) {
            listener.unwatch_device(*object_id);
        }
    }

    for (device_id, object_id) in desired.iter() {
        if watched.contains_key(device_id) {
            continue;
        }
        if let Err(error) = listener.watch_device(*object_id) {
            warn!(
                "{} Could not watch {} for death: {}",
                LOG_TAG.on_cyan().black(),
                device_id,
                error
            );
        }
    }

    *watched = desired;
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
