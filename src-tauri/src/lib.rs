pub mod streaming;
pub mod audio;
pub mod icecast_source;
pub mod streaming_service;
pub mod recording_service;

use streaming::{StreamConfig, StreamManager, StreamMetadata, StreamStatus};
use recording_service::{RecordingService, RecordingConfig, RecordingStatus, RecordingHistoryEntry};
// Re-export audio types for testing and external use
pub use audio::{
    AudioDeviceManager, VirtualMixer, MixerConfig, AudioDeviceInfo, AudioChannel, 
    AudioMetrics, MixerCommand, AudioConfigFactory, EQBand, ThreeBandEqualizer, 
    Compressor, Limiter, PeakDetector, RmsDetector, AudioDatabase, AudioEventBus,
    VULevelData, MasterLevelData, ChannelConfig, OutputRouteConfig, DeviceMonitorStats,
    initialize_device_monitoring, get_device_monitoring_stats as get_monitoring_stats_impl, 
    stop_device_monitoring as stop_monitoring_impl
};
use std::sync::{Arc, Mutex};
use tauri::State;
use tokio::sync::Mutex as AsyncMutex;
use tracing::error;

// Global state management
struct StreamState(Mutex<Option<StreamManager>>);
struct AudioState {
    device_manager: Arc<AsyncMutex<AudioDeviceManager>>,
    mixer: Arc<AsyncMutex<Option<VirtualMixer>>>,
    database: Arc<AudioDatabase>,
    event_bus: Arc<AudioEventBus>,
}
struct RecordingState {
    service: Arc<RecordingService>,
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
    // **CRASH FIX**: Add comprehensive error handling for mixer creation
    println!("🎛️ Creating mixer with {} channels...", config.channels.len());
    
    // Create the mixer with enhanced error handling
    let mut mixer = match VirtualMixer::new(config).await {
        Ok(mixer) => {
            println!("✅ Mixer structure created successfully");
            mixer
        }
        Err(e) => {
            error!("Failed to create mixer: {}", e);
            return Err(format!("Failed to create mixer: {}", e));
        }
    };
    
    // **CRASH FIX**: Start the mixer with better error handling
    match mixer.start().await {
        Ok(()) => {
            println!("✅ Mixer started successfully (always-running mode)");
        }
        Err(e) => {
            error!("Failed to start mixer: {}", e);
            return Err(format!("Failed to start mixer: {}", e));
        }
    }
    
    // Store the initialized mixer
    *audio_state.mixer.lock().await = Some(mixer);
    println!("🎛️ Mixer created, started, and stored successfully");
    Ok(())
}

#[tauri::command]
async fn start_mixer(
    audio_state: State<'_, AudioState>,
) -> Result<(), String> {
    // DEPRECATED: Mixer is now always running after creation
    // This command is kept for compatibility but does nothing
    let mixer_guard = audio_state.mixer.lock().await;
    if mixer_guard.is_some() {
        println!("⚠️ start_mixer called but mixer is already always-running");
        Ok(())
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
async fn stop_mixer(
    audio_state: State<'_, AudioState>,
) -> Result<(), String> {
    // DEPRECATED: Mixer is now always running and cannot be stopped
    // This command is kept for compatibility but does nothing
    let mixer_guard = audio_state.mixer.lock().await;
    if mixer_guard.is_some() {
        println!("⚠️ stop_mixer called but mixer is always-running (operation ignored)");
        Ok(())
    } else {
        Err("No mixer created".to_string())
    }
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
) -> Result<std::collections::HashMap<u32, (f32, f32, f32, f32)>, String> {
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
    // **CRASH FIX**: Validate input device ID
    if new_device_id.trim().is_empty() {
        return Err("Device ID cannot be empty".to_string());
    }
    if new_device_id.len() > 256 {
        return Err("Device ID too long".to_string());
    }
    
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        println!("🔄 Switching input device from {:?} to {}", old_device_id, new_device_id);
        
        // Remove old device if specified
        if let Some(old_id) = old_device_id {
            if !old_id.trim().is_empty() {
                println!("🗑️ Removing old input device: {}", old_id);
                if let Err(e) = mixer.remove_input_stream(&old_id).await {
                    eprintln!("Warning: Failed to remove old input device {}: {}", old_id, e);
                    // Continue anyway - don't fail the entire operation
                }
                // **CRASH FIX**: Add delay to allow cleanup
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
        
        // **CRASH FIX**: Add new device with better error handling
        println!("➕ Adding new input device: {}", new_device_id);
        match mixer.add_input_stream(&new_device_id).await {
            Ok(()) => {
                println!("✅ Successfully switched input device to: {}", new_device_id);
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Failed to add input stream for {}: {}", new_device_id, e);
                Err(format!("Failed to add input device: {}", e))
            }
        }
    } else {
        eprintln!("❌ Cannot switch input device: No mixer has been created yet");
        Err("No mixer created - please create mixer first".to_string())
    }
}

#[tauri::command]
async fn safe_switch_output_device(
    audio_state: State<'_, AudioState>,
    new_device_id: String,
) -> Result<(), String> {
    // **CRASH FIX**: Validate output device ID
    if new_device_id.trim().is_empty() {
        return Err("Device ID cannot be empty".to_string());
    }
    if new_device_id.len() > 256 {
        return Err("Device ID too long".to_string());
    }
    
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        println!("🔊 Switching output device to: {}", new_device_id);
        
        // **CRASH FIX**: Better error handling and logging
        match mixer.set_output_stream(&new_device_id).await {
            Ok(()) => {
                println!("✅ Successfully switched output device to: {}", new_device_id);
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Failed to set output stream for {}: {}", new_device_id, e);
                Err(format!("Failed to set output device: {}", e))
            }
        }
    } else {
        eprintln!("❌ Cannot switch output device: No mixer has been created yet");
        Err("No mixer created - please create mixer first".to_string())
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

// Debug control commands
#[tauri::command]
fn set_audio_debug_enabled(enabled: bool) {
    audio::AUDIO_DEBUG_ENABLED.store(enabled, std::sync::atomic::Ordering::Relaxed);
    println!("🔧 Audio debug logging {}", if enabled { "ENABLED" } else { "DISABLED" });
}

#[tauri::command]
fn get_audio_debug_enabled() -> bool {
    audio::AUDIO_DEBUG_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

// Audio effects management commands
#[tauri::command]
async fn update_channel_eq(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    eq_low_gain: Option<f32>,
    eq_mid_gain: Option<f32>,
    eq_high_gain: Option<f32>,
) -> Result<(), String> {
    let mut mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mut mixer) = *mixer_guard {
        // Clone the current channel first
        let mut updated_channel = if let Some(channel) = mixer.get_channel(channel_id) {
            channel.clone()
        } else {
            return Err(format!("Channel {} not found", channel_id));
        };
        
        // Update EQ settings
        if let Some(gain) = eq_low_gain {
            updated_channel.eq_low_gain = gain.clamp(-12.0, 12.0);
        }
        if let Some(gain) = eq_mid_gain {
            updated_channel.eq_mid_gain = gain.clamp(-12.0, 12.0);
        }
        if let Some(gain) = eq_high_gain {
            updated_channel.eq_high_gain = gain.clamp(-12.0, 12.0);
        }
        
        // Update the channel in the mixer to trigger real-time changes
        mixer.update_channel(channel_id, updated_channel.clone()).await.map_err(|e| e.to_string())?;
        println!("🎛️ Updated EQ for channel {}: low={:.1}, mid={:.1}, high={:.1}", 
            channel_id, updated_channel.eq_low_gain, updated_channel.eq_mid_gain, updated_channel.eq_high_gain);
        
        Ok(())
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
async fn update_channel_compressor(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    threshold: Option<f32>,
    ratio: Option<f32>,
    attack_ms: Option<f32>,
    release_ms: Option<f32>,
    enabled: Option<bool>,
) -> Result<(), String> {
    let mut mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mut mixer) = *mixer_guard {
        // Clone the current channel first
        let mut updated_channel = if let Some(channel) = mixer.get_channel(channel_id) {
            channel.clone()
        } else {
            return Err(format!("Channel {} not found", channel_id));
        };
        
        // Update compressor settings
        if let Some(thresh) = threshold {
            updated_channel.comp_threshold = thresh.clamp(-40.0, 0.0);
        }
        if let Some(r) = ratio {
            updated_channel.comp_ratio = r.clamp(1.0, 10.0);
        }
        if let Some(attack) = attack_ms {
            updated_channel.comp_attack = attack.clamp(0.1, 100.0);
        }
        if let Some(release) = release_ms {
            updated_channel.comp_release = release.clamp(10.0, 1000.0);
        }
        if let Some(en) = enabled {
            updated_channel.comp_enabled = en;
        }
        
        // Update the channel in the mixer to trigger real-time changes
        mixer.update_channel(channel_id, updated_channel.clone()).await.map_err(|e| e.to_string())?;
        println!("🎛️ Updated compressor for channel {}: threshold={:.1}dB, ratio={:.1}:1, attack={:.1}ms, release={:.0}ms, enabled={}", 
            channel_id, updated_channel.comp_threshold, updated_channel.comp_ratio, updated_channel.comp_attack, updated_channel.comp_release, updated_channel.comp_enabled);
        
        Ok(())
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
async fn update_channel_limiter(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    threshold_db: Option<f32>,
    enabled: Option<bool>,
) -> Result<(), String> {
    let mut mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mut mixer) = *mixer_guard {
        // Clone the current channel first
        let mut updated_channel = if let Some(channel) = mixer.get_channel(channel_id) {
            channel.clone()
        } else {
            return Err(format!("Channel {} not found", channel_id));
        };
        
        // Update limiter settings
        if let Some(thresh) = threshold_db {
            updated_channel.limiter_threshold = thresh.clamp(-12.0, 0.0);
        }
        if let Some(en) = enabled {
            updated_channel.limiter_enabled = en;
        }
        
        // Update the channel in the mixer to trigger real-time changes
        mixer.update_channel(channel_id, updated_channel.clone()).await.map_err(|e| e.to_string())?;
        println!("🎛️ Updated limiter for channel {}: threshold={:.1}dB, enabled={}", 
            channel_id, updated_channel.limiter_threshold, updated_channel.limiter_enabled);
        
        Ok(())
    } else {
        Err("No mixer created".to_string())
    }
}

// Effects management commands - add/remove individual effects
#[tauri::command]
async fn add_channel_effect(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    effect_type: String, // "eq", "compressor", "limiter"
) -> Result<(), String> {
    let mut mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mut mixer) = *mixer_guard {
        // Clone the current channel first
        let mut updated_channel = if let Some(channel) = mixer.get_channel(channel_id) {
            channel.clone()
        } else {
            return Err(format!("Channel {} not found", channel_id));
        };
        
        match effect_type.as_str() {
            "eq" => {
                // Reset EQ to flat response (effectively "adding" it)
                updated_channel.eq_low_gain = 0.0;
                updated_channel.eq_mid_gain = 0.0;
                updated_channel.eq_high_gain = 0.0;
                println!("➕ Added EQ to channel {}", channel_id);
            }
            "compressor" => {
                // Enable compressor with default settings
                updated_channel.comp_enabled = true;
                updated_channel.comp_threshold = -12.0;
                updated_channel.comp_ratio = 4.0;
                updated_channel.comp_attack = 10.0;
                updated_channel.comp_release = 100.0;
                println!("➕ Added compressor to channel {}", channel_id);
            }
            "limiter" => {
                // Enable limiter with default settings
                updated_channel.limiter_enabled = true;
                updated_channel.limiter_threshold = -3.0;
                println!("➕ Added limiter to channel {}", channel_id);
            }
            _ => return Err(format!("Unknown effect type: {}", effect_type)),
        }
        
        // Update the channel in the mixer to trigger real-time changes
        mixer.update_channel(channel_id, updated_channel.clone()).await.map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
async fn remove_channel_effect(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    effect_type: String, // "eq", "compressor", "limiter"
) -> Result<(), String> {
    let mut mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mut mixer) = *mixer_guard {
        // Clone the current channel first
        let mut updated_channel = if let Some(channel) = mixer.get_channel(channel_id) {
            channel.clone()
        } else {
            return Err(format!("Channel {} not found", channel_id));
        };
        
        match effect_type.as_str() {
            "eq" => {
                // Reset EQ to flat response (effectively "removing" it)
                updated_channel.eq_low_gain = 0.0;
                updated_channel.eq_mid_gain = 0.0;
                updated_channel.eq_high_gain = 0.0;
                println!("➖ Removed EQ from channel {}", channel_id);
            }
            "compressor" => {
                // Disable compressor
                updated_channel.comp_enabled = false;
                println!("➖ Removed compressor from channel {}", channel_id);
            }
            "limiter" => {
                // Disable limiter
                updated_channel.limiter_enabled = false;
                println!("➖ Removed limiter from channel {}", channel_id);
            }
            _ => return Err(format!("Unknown effect type: {}", effect_type)),
        }
        
        // Update the channel in the mixer to trigger real-time changes
        mixer.update_channel(channel_id, updated_channel.clone()).await.map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
async fn get_channel_effects(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
) -> Result<Vec<String>, String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        if let Some(channel) = mixer.get_channel(channel_id) {
            let mut effects = Vec::new();
            
            // Check which effects are active
            if channel.eq_low_gain != 0.0 || channel.eq_mid_gain != 0.0 || channel.eq_high_gain != 0.0 {
                effects.push("eq".to_string());
            }
            if channel.comp_enabled {
                effects.push("compressor".to_string());
            }
            if channel.limiter_enabled {
                effects.push("limiter".to_string());
            }
            
            Ok(effects)
        } else {
            Err(format!("Channel {} not found", channel_id))
        }
    } else {
        Err("No mixer created".to_string())
    }
}

// Multiple output device management commands
#[tauri::command]
async fn add_output_device(
    audio_state: State<'_, AudioState>,
    device_id: String,
    device_name: String,
    gain: Option<f32>,
    is_monitor: Option<bool>,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        // Create output device configuration
        let output_device = audio::types::OutputDevice {
            device_id: device_id.clone(),
            device_name,
            gain: gain.unwrap_or(1.0),
            enabled: true,
            is_monitor: is_monitor.unwrap_or(false),
        };
        
        // Add the output device directly through the mixer
        mixer.add_output_device(output_device).await.map_err(|e| e.to_string())?;
        println!("✅ Added output device via Tauri command: {}", device_id);
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[tauri::command]
async fn remove_output_device(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        mixer.remove_output_device(&device_id).await.map_err(|e| e.to_string())?;
        println!("✅ Removed output device via Tauri command: {}", device_id);
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[tauri::command]
async fn update_output_device(
    audio_state: State<'_, AudioState>,
    device_id: String,
    device_name: Option<String>,
    gain: Option<f32>,
    enabled: Option<bool>,
    is_monitor: Option<bool>,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        // Get current device configuration
        let current_config = mixer.get_output_device(&device_id).await;
        
        if let Some(mut updated_device) = current_config {
            // Update specified fields
            if let Some(name) = device_name {
                updated_device.device_name = name;
            }
            if let Some(g) = gain {
                updated_device.gain = g;
            }
            if let Some(e) = enabled {
                updated_device.enabled = e;
            }
            if let Some(m) = is_monitor {
                updated_device.is_monitor = m;
            }
            
            mixer.update_output_device(&device_id, updated_device).await.map_err(|e| e.to_string())?;
            println!("✅ Updated output device via Tauri command: {}", device_id);
        } else {
            return Err(format!("Output device not found: {}", device_id));
        }
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[tauri::command]
async fn get_output_devices(
    audio_state: State<'_, AudioState>,
) -> Result<Vec<audio::types::OutputDevice>, String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        Ok(mixer.get_output_devices().await)
    } else {
        Err("No mixer created".to_string())
    }
}

// Device health monitoring commands
#[tauri::command]
async fn get_device_health(
    audio_state: State<'_, AudioState>,
    device_id: String,
) -> Result<Option<audio::devices::DeviceHealth>, String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        Ok(mixer.get_device_health_status(&device_id).await)
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
async fn get_all_device_health(
    audio_state: State<'_, AudioState>,
) -> Result<std::collections::HashMap<String, audio::devices::DeviceHealth>, String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        Ok(mixer.get_all_device_health_statuses().await)
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
async fn report_device_error(
    audio_state: State<'_, AudioState>,
    device_id: String,
    error: String,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        mixer.report_device_error(&device_id, error).await;
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
    // **CRASH FIX**: Validate device ID
    if device_id.trim().is_empty() {
        return Err("Device ID cannot be empty".to_string());
    }
    if device_id.len() > 256 {
        return Err("Device ID too long".to_string());
    }
    
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        // **CRASH FIX**: Use basic add_input_stream for better compatibility
        match mixer.add_input_stream(&device_id).await {
            Ok(()) => {
                println!("✅ Successfully added input stream: {}", device_id);
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Failed to add input stream for {}: {}", device_id, e);
                Err(format!("Failed to add input stream: {}", e))
            }
        }
    } else {
        Err("No mixer created".to_string())
    }
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
    // **CRASH FIX**: Validate device ID
    if device_id.trim().is_empty() {
        return Err("Device ID cannot be empty".to_string());
    }
    if device_id.len() > 256 {
        return Err("Device ID too long".to_string());
    }
    
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        match mixer.set_output_stream(&device_id).await {
            Ok(()) => {
                println!("✅ Successfully set output stream: {}", device_id);
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Failed to set output stream for {}: {}", device_id, e);
                Err(format!("Failed to set output stream: {}", e))
            }
        }
    } else {
        Err("No mixer created".to_string())
    }
}

// Device monitoring commands
#[tauri::command]
async fn start_device_monitoring(
    audio_state: State<'_, AudioState>,
) -> Result<String, String> {
    let mixer_guard = audio_state.mixer.lock().await;
    
    if mixer_guard.is_some() {
        // For now, just return success. The actual device monitoring implementation
        // needs refactoring to work with the app's mixer storage pattern.
        // This is a placeholder until we can properly integrate it.
        println!("✅ Device monitoring started (placeholder implementation)");
        Ok("Device monitoring started successfully (placeholder)".to_string())
    } else {
        Err("No mixer created - cannot start device monitoring".to_string())
    }
}

#[tauri::command]
async fn stop_device_monitoring() -> Result<String, String> {
    match stop_monitoring_impl().await {
        Ok(()) => {
            println!("✅ Device monitoring stopped");
            Ok("Device monitoring stopped successfully".to_string())
        }
        Err(e) => {
            eprintln!("❌ Failed to stop device monitoring: {}", e);
            Err(format!("Failed to stop device monitoring: {}", e))
        }
    }
}

#[tauri::command]
async fn get_device_monitoring_stats() -> Result<Option<DeviceMonitorStats>, String> {
    Ok(get_monitoring_stats_impl().await)
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
        
        // Initialize SQLite database in src-tauri directory for now
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

    // Initialize recording service
    let recording_state = RecordingState {
        service: Arc::new(RecordingService::new()),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(StreamState(Mutex::new(None)))
        .manage(audio_state)
        .manage(recording_state)
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
            add_channel_effect,
            remove_channel_effect,
            get_channel_effects,
            add_input_stream,
            remove_input_stream,
            set_output_stream,
            add_output_device,
            remove_output_device,
            update_output_device,
            get_output_devices,
            get_device_health,
            get_all_device_health,
            report_device_error,
            get_recent_vu_levels,
            get_recent_master_levels,
            save_channel_config,
            load_channel_configs,
            cleanup_old_levels,
            safe_switch_input_device,
            safe_switch_output_device,
            set_audio_debug_enabled,
            get_audio_debug_enabled,
            initialize_icecast_streaming,
            start_icecast_streaming,
            stop_icecast_streaming,
            update_icecast_metadata,
            get_icecast_streaming_status,
            set_stream_bitrate,
            get_available_stream_bitrates,
            get_current_stream_bitrate,
            set_variable_bitrate_streaming,
            get_variable_bitrate_settings,
            start_recording,
            stop_recording,
            get_recording_status,
            save_recording_config,
            get_recording_configs,
            get_recording_history,
            create_default_recording_config,
            select_recording_directory,
            start_device_monitoring,
            stop_device_monitoring,
            get_device_monitoring_stats
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ================================================================================================
// ENHANCED ICECAST STREAMING COMMANDS  
// ================================================================================================

#[tauri::command]
async fn initialize_icecast_streaming(
    server_host: String,
    server_port: u16,
    mount_point: String,
    password: String,
    stream_name: String,
    bitrate: u32,
    state: State<'_, AudioState>
) -> Result<String, String> {
    use crate::streaming_service::{initialize_streaming, connect_streaming_to_mixer, StreamingServiceConfig};
    use crate::icecast_source::{AudioFormat, AudioCodec};
    
    println!("🔧 Initializing Icecast streaming: {}:{}{}", server_host, server_port, mount_point);
    
    // Create streaming configuration
    let config = StreamingServiceConfig {
        server_host: server_host.clone(),
        server_port,
        mount_point: mount_point.clone(),
        password,
        stream_name,
        stream_description: "Live radio stream from Sendin Beats".to_string(),
        stream_genre: "Electronic".to_string(),
        stream_url: "https://sendinbeats.com".to_string(),
        is_public: true,
        audio_format: AudioFormat {
            sample_rate: 48000,
            channels: 2,
            bitrate,
            codec: AudioCodec::Mp3,
        },
        available_bitrates: vec![96, 128, 160, 192, 256, 320],
        selected_bitrate: bitrate,
        enable_variable_bitrate: false,
        vbr_quality: 2,
        auto_reconnect: true,
        max_reconnect_attempts: 5,
        reconnect_delay_ms: 3000,
    };
    
    // Initialize streaming service
    if let Err(e) = initialize_streaming(config).await {
        eprintln!("❌ Failed to initialize streaming service: {}", e);
        return Err(format!("Failed to initialize streaming: {}", e));
    }
    
    // Connect to mixer if available
    if let Some(mixer_ref) = &*state.mixer.lock().await {
        if let Err(e) = connect_streaming_to_mixer(mixer_ref).await {
            eprintln!("❌ Failed to connect streaming to mixer: {}", e);
            return Err(format!("Failed to connect to mixer: {}", e));
        }
        println!("✅ Streaming service connected to mixer");
    } else {
        println!("⚠️ No mixer available - streaming initialized but not connected");
    }
    
    Ok(format!("Icecast streaming initialized: {}:{}{}", server_host, server_port, mount_point))
}

#[tauri::command]
async fn start_icecast_streaming() -> Result<String, String> {
    use crate::streaming_service::start_streaming;
    
    println!("🎯 Starting Icecast streaming...");
    
    match start_streaming().await {
        Ok(()) => {
            println!("✅ Icecast streaming started successfully");
            Ok("Streaming started successfully".to_string())
        }
        Err(e) => {
            eprintln!("❌ Failed to start streaming: {}", e);
            Err(format!("Failed to start streaming: {}", e))
        }
    }
}

#[tauri::command]
async fn stop_icecast_streaming() -> Result<String, String> {
    use crate::streaming_service::stop_streaming;
    
    println!("🛑 Stopping Icecast streaming...");
    
    match stop_streaming().await {
        Ok(()) => {
            println!("✅ Icecast streaming stopped successfully");
            Ok("Streaming stopped successfully".to_string())
        }
        Err(e) => {
            eprintln!("❌ Failed to stop streaming: {}", e);
            Err(format!("Failed to stop streaming: {}", e))
        }
    }
}

#[tauri::command]
async fn update_icecast_metadata(title: String, artist: String) -> Result<String, String> {
    use crate::streaming_service::update_stream_metadata;
    
    println!("📝 Updating stream metadata: {} - {}", artist, title);
    
    match update_stream_metadata(title.clone(), artist.clone()).await {
        Ok(()) => {
            println!("✅ Stream metadata updated successfully");
            Ok(format!("Metadata updated: {} - {}", artist, title))
        }
        Err(e) => {
            eprintln!("❌ Failed to update metadata: {}", e);
            Err(format!("Failed to update metadata: {}", e))
        }
    }
}

#[tauri::command]
async fn get_icecast_streaming_status() -> Result<serde_json::Value, String> {
    use crate::streaming_service::get_streaming_status;
    
    let status = get_streaming_status().await;
    
    match serde_json::to_value(&status) {
        Ok(json_status) => Ok(json_status),
        Err(e) => {
            eprintln!("❌ Failed to serialize streaming status: {}", e);
            Err(format!("Failed to get status: {}", e))
        }
    }
}

#[tauri::command]
async fn set_stream_bitrate(bitrate: u32) -> Result<String, String> {
    use crate::streaming_service::set_stream_bitrate;
    
    println!("🎵 Setting stream bitrate to {}kbps", bitrate);
    
    match set_stream_bitrate(bitrate).await {
        Ok(()) => {
            println!("✅ Bitrate set to {}kbps (restart streaming to apply)", bitrate);
            Ok(format!("Bitrate set to {}kbps", bitrate))
        }
        Err(e) => {
            eprintln!("❌ Failed to set bitrate: {}", e);
            Err(format!("Failed to set bitrate: {}", e))
        }
    }
}

#[tauri::command]
async fn get_available_stream_bitrates() -> Result<Vec<u32>, String> {
    use crate::streaming_service::get_available_bitrates;
    
    let bitrates = get_available_bitrates().await;
    Ok(bitrates)
}

#[tauri::command]
async fn get_current_stream_bitrate() -> Result<u32, String> {
    use crate::streaming_service::get_current_stream_bitrate;
    
    let bitrate = get_current_stream_bitrate().await;
    Ok(bitrate)
}

#[tauri::command]
async fn set_variable_bitrate_streaming(enabled: bool, quality: u8) -> Result<String, String> {
    use crate::streaming_service::set_variable_bitrate_streaming;
    
    println!("🎵 Setting variable bitrate: enabled={}, quality=V{}", enabled, quality);
    
    match set_variable_bitrate_streaming(enabled, quality).await {
        Ok(()) => {
            println!("✅ Variable bitrate set: enabled={}, quality=V{}", enabled, quality);
            Ok(format!("Variable bitrate set: enabled={}, quality=V{}", enabled, quality))
        }
        Err(e) => {
            eprintln!("❌ Failed to set variable bitrate: {}", e);
            Err(format!("Failed to set variable bitrate: {}", e))
        }
    }
}

#[tauri::command]
async fn get_variable_bitrate_settings() -> Result<(bool, u8), String> {
    use crate::streaming_service::get_variable_bitrate_settings;
    
    let (enabled, quality) = get_variable_bitrate_settings().await;
    Ok((enabled, quality))
}

// ================================================================================================
// RECORDING SERVICE COMMANDS
// ================================================================================================

#[tauri::command]
async fn start_recording(
    recording_state: State<'_, RecordingState>,
    audio_state: State<'_, AudioState>,
    config: RecordingConfig,
) -> Result<String, String> {
    println!("🎙️ Starting recording with config: {}", config.name);
    
    // Get audio output receiver from mixer
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        let audio_rx = mixer.get_audio_output_receiver();
        
        match recording_state.service.start_recording(config, audio_rx).await {
            Ok(session_id) => {
                println!("✅ Recording started with session ID: {}", session_id);
                Ok(session_id)
            }
            Err(e) => {
                eprintln!("❌ Failed to start recording: {}", e);
                Err(format!("Failed to start recording: {}", e))
            }
        }
    } else {
        Err("No mixer available - please create mixer first".to_string())
    }
}

#[tauri::command]
async fn stop_recording(
    recording_state: State<'_, RecordingState>,
) -> Result<Option<RecordingHistoryEntry>, String> {
    println!("🛑 Stopping recording...");
    
    match recording_state.service.stop_recording().await {
        Ok(history_entry) => {
            if let Some(ref entry) = history_entry {
                println!("✅ Recording stopped: {:?}", entry.file_path);
            } else {
                println!("⚠️ No active recording to stop");
            }
            Ok(history_entry)
        }
        Err(e) => {
            eprintln!("❌ Failed to stop recording: {}", e);
            Err(format!("Failed to stop recording: {}", e))
        }
    }
}

#[tauri::command]
async fn get_recording_status(
    recording_state: State<'_, RecordingState>,
) -> Result<RecordingStatus, String> {
    Ok(recording_state.service.get_status().await)
}

#[tauri::command]
async fn save_recording_config(
    recording_state: State<'_, RecordingState>,
    config: RecordingConfig,
) -> Result<String, String> {
    println!("💾 Saving recording config: {}", config.name);
    
    match recording_state.service.save_config(config.clone()).await {
        Ok(()) => {
            println!("✅ Recording config saved: {}", config.name);
            Ok(format!("Config '{}' saved successfully", config.name))
        }
        Err(e) => {
            eprintln!("❌ Failed to save recording config: {}", e);
            Err(format!("Failed to save config: {}", e))
        }
    }
}

#[tauri::command]
async fn get_recording_configs(
    recording_state: State<'_, RecordingState>,
) -> Result<Vec<RecordingConfig>, String> {
    Ok(recording_state.service.get_configs().await)
}

#[tauri::command]
async fn get_recording_history(
    recording_state: State<'_, RecordingState>,
) -> Result<Vec<RecordingHistoryEntry>, String> {
    Ok(recording_state.service.get_history().await)
}

#[tauri::command]
async fn create_default_recording_config() -> Result<RecordingConfig, String> {
    Ok(RecordingConfig::default())
}

#[tauri::command]
async fn select_recording_directory() -> Result<Option<String>, String> {
    use std::path::PathBuf;
    
    // For now, return the default Music directory path
    // In a full implementation, this would show a native directory picker
    let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
    let music_dir = home_dir.join("Music");
    
    // Return the Music directory as default for now
    // TODO: Implement actual directory picker dialog
    Ok(Some(music_dir.to_string_lossy().to_string()))
}
