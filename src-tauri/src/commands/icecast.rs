use tauri::State;
use crate::AudioState;

// ================================================================================================
// ENHANCED ICECAST STREAMING COMMANDS  
// ================================================================================================

#[tauri::command]
pub async fn initialize_icecast_streaming(
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
pub async fn start_icecast_streaming() -> Result<String, String> {
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
pub async fn stop_icecast_streaming() -> Result<String, String> {
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
pub async fn update_icecast_metadata(title: String, artist: String) -> Result<String, String> {
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
pub async fn get_icecast_streaming_status() -> Result<serde_json::Value, String> {
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
pub async fn set_stream_bitrate(bitrate: u32) -> Result<String, String> {
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
pub async fn get_available_stream_bitrates() -> Result<Vec<u32>, String> {
    use crate::streaming_service::get_available_bitrates;
    
    let bitrates = get_available_bitrates().await;
    Ok(bitrates)
}

#[tauri::command]
pub async fn get_current_stream_bitrate() -> Result<u32, String> {
    use crate::streaming_service::get_current_stream_bitrate;
    
    let bitrate = get_current_stream_bitrate().await;
    Ok(bitrate)
}

#[tauri::command]
pub async fn set_variable_bitrate_streaming(enabled: bool, quality: u8) -> Result<String, String> {
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
pub async fn get_variable_bitrate_settings() -> Result<(bool, u8), String> {
    use crate::streaming_service::get_variable_bitrate_settings;
    
    let (enabled, quality) = get_variable_bitrate_settings().await;
    Ok((enabled, quality))
}