// Bringing a device up on the mixer: resolve a handle for it, record its
// configuration, and hand the stream to the audio thread.
//
// The user-driven device switch and the device watcher's reconnect recovery
// both need this, so it lives apart from either caller.

use colored::*;
use tokio::sync::oneshot;
use tracing::{error, info, warn};

use crate::audio::mixer::stream_management::AudioCommand;
use crate::audio::types::AudioDeviceHandle;
use crate::entities::configured_audio_device::Model as ConfiguredAudioDevice;
use crate::AudioState;

/// What the database needs to know about a device before its stream exists.
struct DeviceDescription {
    name: String,
    sample_rate: u32,
    channels: u16,
}

/// Undo the database entry written ahead of a device connection that then failed.
///
/// `inserted_this_call` guards against deleting a configuration that predates
/// this call: a device that is merely unplugged must survive to be restored
/// once it comes back.
pub async fn rollback_device_configuration(
    audio_state: &AudioState,
    device_id: &str,
    inserted_this_call: bool,
) {
    if !inserted_this_call {
        info!(
            "{}: Keeping existing configuration for '{}' after connection failure",
            "CONFIG_RETAINED".on_blue().magenta(),
            device_id
        );
        return;
    }

    if let Err(cleanup_err) =
        crate::commands::configurations::remove_device_configuration(audio_state, device_id).await
    {
        warn!(
            "Failed to clean up device configuration for '{}': {}",
            device_id, cleanup_err
        );
    }
}

/// Connect `device_id` as an input and record it against `channel_number`.
///
/// Safe to call for a device that already has a configuration row - the
/// database write finds the existing row rather than duplicating it - which is
/// what lets reconnect replay reuse this path.
pub async fn attach_input_device(
    state: &AudioState,
    device_id: &str,
    is_app_audio: bool,
    channel_number: Option<i32>,
) -> Result<Option<ConfiguredAudioDevice>, String> {
    let (device_handle, description) =
        resolve_input_device_handle(state, device_id, is_app_audio).await?;

    // The database entry has to exist before the command is sent: the audio
    // pipeline queries it for the channel number while setting the device up.
    let device_configuration = match description {
        Some(description) => {
            create_configuration(state, device_id, &description, is_app_audio, channel_number)
                .await?
        }
        None => None,
    };

    // Only roll back a row this call inserted. A pre-existing row belongs to a
    // configuration the user saved earlier, and a transient failure to connect
    // the device is no reason to discard it.
    let inserted_this_call = device_configuration
        .as_ref()
        .is_some_and(|outcome| outcome.created);
    let created_device_model = device_configuration.map(|outcome| outcome.model);

    let (command, response_rx) = build_add_input_command(device_id, device_handle)?;

    if let Err(e) = state.audio_command_tx.send(command).await {
        let error_msg = format!("Audio system not available - failed to send command: {}", e);
        error!("{}", error_msg);
        return Err(error_msg);
    }

    match response_rx.await {
        Ok(Ok(())) => {
            info!("✅ Successfully added input device: {}", device_id);
            Ok(created_device_model)
        }
        Ok(Err(e)) => {
            error!("Failed to add input device to audio pipeline: {}", e);
            rollback_device_configuration(state, device_id, inserted_this_call).await;
            Err(format!("Failed to add input device: {}", e))
        }
        Err(_) => {
            error!(
                "Audio system did not respond while adding input device '{}'",
                device_id
            );
            rollback_device_configuration(state, device_id, inserted_this_call).await;
            Err("Audio system did not respond".to_string())
        }
    }
}

/// Connect `device_id` as an output.
///
/// Unlike the input path this writes nothing to the database - an output's
/// configuration row is created by the switch command once the stream is live,
/// so reconnect only has to rebuild the stream.
pub async fn attach_output_device(state: &AudioState, device_id: &str) -> Result<(), String> {
    let device_handle = {
        let device_manager = state.device_manager.lock().await;
        device_manager
            .find_audio_device(device_id, false) // false = output device
            .await
            .map_err(|e| format!("Failed to find output device {}: {}", device_id, e))?
    };

    let (response_tx, response_rx) = oneshot::channel();

    let command = match device_handle {
        #[cfg(target_os = "macos")]
        AudioDeviceHandle::CoreAudio(coreaudio_device) => AudioCommand::AddCoreAudioOutputStream {
            device_id: device_id.to_string(),
            coreaudio_device,
            response_tx,
        },
        AudioDeviceHandle::FilePlayer(_) => {
            return Err("File players are input-only and cannot be used as outputs".to_string());
        }
        #[cfg(target_os = "macos")]
        AudioDeviceHandle::ApplicationAudio(_) => {
            return Err(
                "Application audio devices are input-only and cannot be used as outputs"
                    .to_string(),
            );
        }
        #[cfg(not(target_os = "macos"))]
        _ => return Err("Unsupported device type for this platform".to_string()),
    };

    if let Err(e) = state.audio_command_tx.send(command).await {
        let error_msg = format!("Audio system not available - failed to send command: {}", e);
        error!("{}", error_msg);
        return Err(error_msg);
    }

    match response_rx.await {
        Ok(Ok(())) => {
            info!("✅ Successfully set output stream: {}", device_id);
            Ok(())
        }
        Ok(Err(e)) => Err(format!("Failed to set output stream: {}", e)),
        Err(_) => Err("Audio system did not respond".to_string()),
    }
}

/// Application sources are keyed by bundle identifier and resolved to whatever
/// PID that application holds right now; everything else comes from the device
/// manager.
async fn resolve_input_device_handle(
    state: &AudioState,
    device_id: &str,
    is_app_audio: bool,
) -> Result<(AudioDeviceHandle, Option<DeviceDescription>), String> {
    if is_app_audio {
        #[cfg(target_os = "macos")]
        {
            let source_identifier = device_id
                .strip_prefix("app-")
                .ok_or_else(|| format!("Invalid application audio device ID: {}", device_id))?;

            let app_info =
                crate::audio::screencapture::resolve_application_source(source_identifier)
                    .map_err(|e| {
                        error!(
                            "{}: Cannot capture '{}': {}",
                            "APP_SOURCE_UNAVAILABLE".on_red().bright_white(),
                            source_identifier,
                            e
                        );
                        format!(
                            "Application '{}' is not available: {}",
                            source_identifier, e
                        )
                    })?;

            let device_handle =
                AudioDeviceHandle::ApplicationAudio(crate::audio::types::ApplicationAudioDevice {
                    pid: app_info.pid as u32,
                    name: app_info.application_name.clone(),
                    sample_rate: crate::types::DEFAULT_SAMPLE_RATE,
                    channels: 2,
                });

            return Ok((
                device_handle,
                Some(DeviceDescription {
                    name: app_info.application_name,
                    sample_rate: crate::types::DEFAULT_SAMPLE_RATE,
                    channels: 2,
                }),
            ));
        }
        #[cfg(not(target_os = "macos"))]
        {
            return Err("Application audio not supported on this platform".to_string());
        }
    }

    // Checked before the device manager is asked: a player is not hardware and
    // enumerating for it would only ever come back empty.
    if let Some(player_device) = state.file_player_manager.get_player(device_id) {
        let player = player_device.get_player();
        let (sample_rate, channels) = player.output_format();

        let handle = AudioDeviceHandle::FilePlayer(crate::audio::types::FilePlayerInputDevice {
            player,
            name: player_device.get_device_name().to_string(),
            sample_rate,
            channels,
        });

        let description = DeviceDescription {
            name: player_device.get_device_name().to_string(),
            sample_rate,
            channels,
        };

        return Ok((handle, Some(description)));
    }

    let device_manager = state.device_manager.lock().await;
    let device_handle = device_manager
        .find_audio_device(device_id, true) // true = input device
        .await
        .map_err(|e| format!("Failed to find input device {}: {}", device_id, e))?;

    let description = match &device_handle {
        #[cfg(target_os = "macos")]
        AudioDeviceHandle::CoreAudio(coreaudio_device) => Some(DeviceDescription {
            name: coreaudio_device.name.clone(),
            sample_rate: coreaudio_device.sample_rate,
            channels: coreaudio_device.channels,
        }),
        #[cfg(target_os = "macos")]
        AudioDeviceHandle::ApplicationAudio(_) => None,
        // A player has no hardware to describe, but it does need a row: that is
        // what gives it a channel strip and somewhere to keep its gain.
        AudioDeviceHandle::FilePlayer(file_player) => Some(DeviceDescription {
            name: file_player.name.clone(),
            sample_rate: file_player.sample_rate,
            channels: file_player.channels,
        }),
        #[cfg(not(target_os = "macos"))]
        _ => None,
    };

    Ok((device_handle, description))
}

async fn create_configuration(
    state: &AudioState,
    device_id: &str,
    description: &DeviceDescription,
    is_app_audio: bool,
    channel_number: Option<i32>,
) -> Result<Option<crate::commands::configurations::DeviceConfigurationOutcome>, String> {
    crate::commands::configurations::create_device_configuration(
        state,
        device_id,
        &description.name,
        description.sample_rate as i32,
        description.channels as u32,
        true, // is_input
        is_app_audio || device_id.contains("BlackHole") || device_id.contains("SoundflowerBed"),
        channel_number,
    )
    .await
    .map_err(|e| format!("Failed to create device configuration in database: {}", e))
}

fn build_add_input_command(
    device_id: &str,
    device_handle: AudioDeviceHandle,
) -> Result<(AudioCommand, oneshot::Receiver<anyhow::Result<()>>), String> {
    let buffer_capacity = 96000;
    let (producer, _consumer) = rtrb::RingBuffer::<f32>::new(buffer_capacity);
    let (response_tx, response_rx) = oneshot::channel();

    let command = match device_handle {
        #[cfg(target_os = "macos")]
        AudioDeviceHandle::CoreAudio(coreaudio_device) => AudioCommand::AddCoreAudioInputStream {
            device_id: device_id.to_string(),
            coreaudio_device_id: coreaudio_device.device_id,
            device_name: coreaudio_device.name.clone(),
            channels: coreaudio_device.channels,
            producer,
            response_tx,
        },
        #[cfg(target_os = "macos")]
        AudioDeviceHandle::ApplicationAudio(app_device) => {
            AudioCommand::AddApplicationAudioInputStream {
                device_id: device_id.to_string(),
                pid: app_device.pid,
                device_name: app_device.name.clone(),
                channels: app_device.channels,
                producer,
                response_tx,
            }
        }
        // No producer: the queue between the decoder and the pipeline is made on
        // the audio thread, since only it knows the chunk the worker will read.
        AudioDeviceHandle::FilePlayer(file_player) => AudioCommand::AddFilePlayerInputStream {
            device_id: device_id.to_string(),
            player: file_player.player.clone(),
            device_name: file_player.name.clone(),
            sample_rate: file_player.sample_rate,
            channels: file_player.channels,
            response_tx,
        },
        #[cfg(not(target_os = "macos"))]
        _ => return Err("Unsupported device type for this platform".to_string()),
    };

    Ok((command, response_rx))
}
