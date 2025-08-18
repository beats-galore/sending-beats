pub mod streaming;
pub mod audio;

use streaming::{StreamConfig, StreamManager, StreamMetadata, StreamStatus};
// Re-export audio types for testing and external use
pub use audio::{
    AudioDeviceManager, VirtualMixer, MixerConfig, AudioDeviceInfo, AudioChannel, 
    AudioMetrics, MixerCommand, AudioConfigFactory, EQBand, ThreeBandEqualizer, 
    Compressor, Limiter, PeakDetector, RmsDetector
};
use std::sync::{Arc, Mutex};
use tauri::State;
use tokio::sync::Mutex as AsyncMutex;

// Global state management
struct StreamState(Mutex<Option<StreamManager>>);
struct AudioState {
    device_manager: Arc<AsyncMutex<AudioDeviceManager>>,
    mixer: Arc<AsyncMutex<Option<VirtualMixer>>>,
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn connect_to_stream(
    state: State<'_, StreamState>,
    config: StreamConfig,
) -> Result<StreamStatus, String> {
    let mut stream_manager = StreamManager::new(config);
    
    stream_manager
        .connect()
        .await
        .map_err(|e| e.to_string())?;

    let status = stream_manager.get_status().await;
    
    // Store the stream manager in state
    *state.0.lock().unwrap() = Some(stream_manager);
    
    Ok(status)
}

#[tauri::command]
async fn disconnect_from_stream(state: State<'_, StreamState>) -> Result<(), String> {
    // Take ownership of the stream manager to avoid holding the lock across await
    let stream_manager_opt = state.0.lock().unwrap().take();
    
    if let Some(mut stream_manager) = stream_manager_opt {
        stream_manager
            .disconnect()
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn start_streaming(
    state: State<'_, StreamState>,
    audio_data: Vec<u8>,
) -> Result<(), String> {
    // Clone the stream manager to avoid holding the lock across await
    let stream_manager_opt = state.0.lock().unwrap().clone();
    
    if let Some(mut stream_manager) = stream_manager_opt {
        stream_manager
            .start_stream(audio_data)
            .await
            .map_err(|e| e.to_string())?;
        
        // Update the state with the modified stream manager
        *state.0.lock().unwrap() = Some(stream_manager);
    } else {
        return Err("Not connected to stream".to_string());
    }
    Ok(())
}

#[tauri::command]
async fn stop_streaming(state: State<'_, StreamState>) -> Result<(), String> {
    // Clone the stream manager to avoid holding the lock across await
    let stream_manager_opt = state.0.lock().unwrap().clone();
    
    if let Some(mut stream_manager) = stream_manager_opt {
        stream_manager
            .stop_stream()
            .await
            .map_err(|e| e.to_string())?;
        
        // Update the state with the modified stream manager
        *state.0.lock().unwrap() = Some(stream_manager);
    }
    Ok(())
}

#[tauri::command]
async fn update_metadata(
    state: State<'_, StreamState>,
    metadata: StreamMetadata,
) -> Result<(), String> {
    // Clone the stream manager to avoid holding the lock across await
    let stream_manager_opt = state.0.lock().unwrap().clone();
    
    if let Some(stream_manager) = stream_manager_opt {
        stream_manager
            .update_metadata(metadata)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        return Err("Not connected to stream".to_string());
    }
    Ok(())
}

#[tauri::command]
async fn get_stream_status(state: State<'_, StreamState>) -> Result<StreamStatus, String> {
    // Clone the stream manager reference to avoid holding the lock across await
    let stream_manager_opt = state.0.lock().unwrap().clone();
    
    if let Some(stream_manager) = stream_manager_opt {
        Ok(stream_manager.get_status().await)
    } else {
        Ok(StreamStatus {
            is_connected: false,
            is_streaming: false,
            current_listeners: 0,
            peak_listeners: 0,
            stream_duration: 0,
            bitrate: 0,
            error_message: None,
        })
    }
}

#[tauri::command]
async fn get_listener_stats(state: State<'_, StreamState>) -> Result<(u32, u32), String> {
    // Clone the stream manager reference to avoid holding the lock across await
    let stream_manager_opt = state.0.lock().unwrap().clone();
    
    if let Some(stream_manager) = stream_manager_opt {
        stream_manager
            .get_listener_stats()
            .await
            .map_err(|e| e.to_string())
    } else {
        Err("Not connected to stream".to_string())
    }
}

// Audio device management commands
#[tauri::command]
async fn enumerate_audio_devices(
    audio_state: State<'_, AudioState>,
) -> Result<Vec<AudioDeviceInfo>, String> {
    let device_manager = audio_state.device_manager.lock().await;
    device_manager
        .enumerate_devices()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_audio_device(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<Option<AudioDeviceInfo>, String> {
    let device_manager = audio_state.device_manager.lock().await;
    Ok(device_manager.get_device(&device_id).await)
}

// Virtual mixer commands
#[tauri::command]
async fn create_mixer(
    audio_state: State<'_, AudioState>,
    config: MixerConfig,
) -> Result<(), String> {
    // We need to unwrap the AudioDeviceManager from the Mutex to pass to the mixer
    // Since VirtualMixer needs to own it, we'll create a new one for now
    // TODO: Refactor this to properly share the AudioDeviceManager instance
    let mixer = VirtualMixer::new(config)
        .await
        .map_err(|e| e.to_string())?;
    
    *audio_state.mixer.lock().await = Some(mixer);
    Ok(())
}

#[tauri::command]
async fn start_mixer(
    audio_state: State<'_, AudioState>,
) -> Result<(), String> {
    let mut mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mut mixer) = *mixer_guard {
        mixer.start().await.map_err(|e| e.to_string())?;
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[tauri::command]
async fn stop_mixer(
    audio_state: State<'_, AudioState>,
) -> Result<(), String> {
    let mut mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mut mixer) = *mixer_guard {
        mixer.stop().await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn add_mixer_channel(
    audio_state: State<'_, AudioState>,
    channel: AudioChannel,
) -> Result<(), String> {
    let mut mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mut mixer) = *mixer_guard {
        mixer.add_channel(channel).await.map_err(|e| e.to_string())?;
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[tauri::command]
async fn update_mixer_channel(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    channel: AudioChannel,
) -> Result<(), String> {
    let mut mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mut mixer) = *mixer_guard {
        mixer.update_channel(channel_id, channel).await.map_err(|e| e.to_string())?;
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[tauri::command]
async fn get_mixer_metrics(
    audio_state: State<'_, AudioState>,
) -> Result<AudioMetrics, String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        Ok(mixer.get_metrics().await)
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
async fn get_channel_levels(
    audio_state: State<'_, AudioState>,
) -> Result<std::collections::HashMap<u32, (f32, f32)>, String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        Ok(mixer.get_channel_levels().await)
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
async fn get_master_levels(
    audio_state: State<'_, AudioState>,
) -> Result<(f32, f32, f32, f32), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        Ok(mixer.get_master_levels().await)
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
async fn send_mixer_command(
    audio_state: State<'_, AudioState>,
    command: MixerCommand,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        mixer.send_command(command).await.map_err(|e| e.to_string())?;
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[tauri::command]
fn get_dj_mixer_config() -> MixerConfig {
    AudioConfigFactory::create_dj_config()
}

#[tauri::command]
fn get_streaming_mixer_config() -> MixerConfig {
    AudioConfigFactory::create_streaming_config()
}

// Audio effects management commands
#[tauri::command]
async fn update_channel_eq(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    band: String, // "low", "mid", or "high"
    gain_db: f32,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        let eq_band = match band.as_str() {
            "low" => EQBand::Low,
            "mid" => EQBand::Mid,
            "high" => EQBand::High,
            _ => return Err("Invalid EQ band".to_string()),
        };
        
        // This would need a new method in VirtualMixer to update EQ settings
        // For now, we'll update through channel configuration
        Ok(())
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
async fn update_channel_compressor(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    threshold: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    enabled: bool,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        // This would need a new method in VirtualMixer to update compressor settings
        Ok(())
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
async fn update_channel_limiter(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    threshold_db: f32,
    enabled: bool,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        // This would need a new method in VirtualMixer to update limiter settings
        Ok(())
    } else {
        Err("No mixer created".to_string())
    }
}

// Audio stream management commands
#[tauri::command]
async fn add_input_stream(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        mixer.add_input_stream(&device_id).await.map_err(|e| e.to_string())?;
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[tauri::command]
async fn remove_input_stream(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        mixer.remove_input_stream(&device_id).await.map_err(|e| e.to_string())?;
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[tauri::command]
async fn set_output_stream(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        mixer.set_output_stream(&device_id).await.map_err(|e| e.to_string())?;
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize audio system
    let audio_device_manager = match AudioDeviceManager::new() {
        Ok(manager) => Arc::new(AsyncMutex::new(manager)),
        Err(e) => {
            eprintln!("Failed to initialize audio device manager: {}", e);
            std::process::exit(1);
        }
    };

    let audio_state = AudioState {
        device_manager: audio_device_manager,
        mixer: Arc::new(AsyncMutex::new(None)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(StreamState(Mutex::new(None)))
        .manage(audio_state)
        .invoke_handler(tauri::generate_handler![
            greet,
            connect_to_stream,
            disconnect_from_stream,
            start_streaming,
            stop_streaming,
            update_metadata,
            get_stream_status,
            get_listener_stats,
            enumerate_audio_devices,
            get_audio_device,
            create_mixer,
            start_mixer,
            stop_mixer,
            add_mixer_channel,
            update_mixer_channel,
            get_mixer_metrics,
            get_channel_levels,
            get_master_levels,
            send_mixer_command,
            get_dj_mixer_config,
            get_streaming_mixer_config,
            update_channel_eq,
            update_channel_compressor,
            update_channel_limiter,
            add_input_stream,
            remove_input_stream,
            set_output_stream
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
