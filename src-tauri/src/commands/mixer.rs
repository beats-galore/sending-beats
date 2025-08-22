use tauri::State;
use tracing::error;
use crate::{AudioState, VirtualMixer, MixerConfig, AudioChannel, AudioMetrics, MixerCommand, AudioConfigFactory};

// Virtual mixer commands
#[tauri::command]
pub async fn create_mixer(
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
pub async fn start_mixer(
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
pub async fn stop_mixer(
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
pub async fn add_mixer_channel(
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
pub async fn update_mixer_channel(
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
pub async fn get_mixer_metrics(
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
pub async fn get_channel_levels(
    audio_state: State<'_, AudioState>,
) -> Result<std::collections::HashMap<u32, (f32, f32, f32, f32)>, String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        Ok(mixer.get_channel_levels().await)
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
pub async fn get_master_levels(
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
pub async fn send_mixer_command(
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
pub fn get_dj_mixer_config() -> MixerConfig {
    AudioConfigFactory::create_dj_config()
}

#[tauri::command]
pub fn get_streaming_mixer_config() -> MixerConfig {
    AudioConfigFactory::create_streaming_config()
}