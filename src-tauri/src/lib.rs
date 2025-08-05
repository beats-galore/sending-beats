mod streaming;

use streaming::{StreamConfig, StreamManager, StreamMetadata, StreamStatus};
use std::sync::Mutex;
use tauri::State;

// Global stream manager state
struct StreamState(Mutex<Option<StreamManager>>);

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(StreamState(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            greet,
            connect_to_stream,
            disconnect_from_stream,
            start_streaming,
            stop_streaming,
            update_metadata,
            get_stream_status,
            get_listener_stats
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
