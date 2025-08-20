pub mod streaming;
pub mod audio;

use streaming::{StreamConfig, StreamManager, StreamMetadata, StreamStatus};
// Re-export audio types for testing and external use
pub use audio::{
    AudioDeviceManager, VirtualMixer, MixerConfig, AudioDeviceInfo, AudioChannel, 
    AudioMetrics, MixerCommand, AudioConfigFactory, EQBand, ThreeBandEqualizer, 
    Compressor, Limiter, PeakDetector, RmsDetector, AudioDatabase, AudioEventBus,
    VULevelData, MasterLevelData, ChannelConfig, OutputRouteConfig
};
use std::sync::{Arc, Mutex};
use tauri::State;
use tokio::sync::Mutex as AsyncMutex;

// Global state management
struct StreamState(Mutex<Option<StreamManager>>);
struct AudioState {
    device_manager: Arc<AsyncMutex<AudioDeviceManager>>,
    mixer: Arc<AsyncMutex<Option<VirtualMixer>>>,
    database: Arc<AudioDatabase>,
    event_bus: Arc<AudioEventBus>,
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
async fn refresh_audio_devices(
    audio_state: State<'_, AudioState>,
) -> Result<Vec<AudioDeviceInfo>, String> {
    let device_manager = audio_state.device_manager.lock().await;
    // Force a fresh device enumeration
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

// SQLite-based VU meter commands for improved performance
#[tauri::command]
async fn get_recent_vu_levels(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    limit: Option<i64>,
) -> Result<Vec<VULevelData>, String> {
    let limit = limit.unwrap_or(50); // Default to last 50 readings
    audio_state.database
        .get_recent_vu_levels(channel_id, limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_recent_master_levels(
    audio_state: State<'_, AudioState>,
    limit: Option<i64>,
) -> Result<Vec<MasterLevelData>, String> {
    let limit = limit.unwrap_or(50); // Default to last 50 readings
    audio_state.database
        .get_recent_master_levels(limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_channel_config(
    audio_state: State<'_, AudioState>,
    channel: ChannelConfig,
) -> Result<u32, String> {
    audio_state.database
        .save_channel_config(&channel)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_channel_configs(
    audio_state: State<'_, AudioState>,
) -> Result<Vec<ChannelConfig>, String> {
    audio_state.database
        .load_channel_configs()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cleanup_old_levels(
    audio_state: State<'_, AudioState>,
) -> Result<u64, String> {
    audio_state.database
        .cleanup_old_vu_levels()
        .await
        .map_err(|e| e.to_string())
}

// Crash-safe device switching commands
#[tauri::command]
async fn safe_switch_input_device(
    audio_state: State<'_, AudioState>,
    old_device_id: Option<String>,
    new_device_id: String,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        // Remove old device if specified
        if let Some(old_id) = old_device_id {
            if let Err(e) = mixer.remove_input_stream(&old_id).await {
                eprintln!("Warning: Failed to remove old input device {}: {}", old_id, e);
                // Continue anyway - don't fail the entire operation
            }
        }
        
        // Add new device
        mixer.add_input_stream(&new_device_id).await.map_err(|e| e.to_string())
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
async fn safe_switch_output_device(
    audio_state: State<'_, AudioState>,
    new_device_id: String,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        mixer.set_output_stream(&new_device_id).await.map_err(|e| e.to_string())
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
    // Initialize the Tokio runtime for database initialization
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    
    let audio_state = rt.block_on(async {
        // Initialize audio system
        let audio_device_manager = match AudioDeviceManager::new() {
            Ok(manager) => Arc::new(AsyncMutex::new(manager)),
            Err(e) => {
                eprintln!("Failed to initialize audio device manager: {}", e);
                std::process::exit(1);
            }
        };
        
        // Initialize SQLite database
        let database_path = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("data")
            .join("sendin_beats.db");
            
        println!("🗄️  Initializing database at: {}", database_path.display());
        
        let database = match AudioDatabase::new(&database_path).await {
            Ok(db) => Arc::new(db),
            Err(e) => {
                eprintln!("Failed to initialize database: {}", e);
                std::process::exit(1);
            }
        };
        
        // Initialize event bus for lock-free audio data transfer
        let event_bus = Arc::new(AudioEventBus::new(1000)); // Buffer up to 1000 events
        
        println!("✅ Audio system initialization complete");
        
        AudioState {
            device_manager: audio_device_manager,
            mixer: Arc::new(AsyncMutex::new(None)),
            database,
            event_bus,
        }
    });

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
            refresh_audio_devices,
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
            set_output_stream,
            get_recent_vu_levels,
            get_recent_master_levels,
            save_channel_config,
            load_channel_configs,
            cleanup_old_levels,
            safe_switch_input_device,
            safe_switch_output_device
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
