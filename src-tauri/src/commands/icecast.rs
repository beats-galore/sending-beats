use crate::audio::broadcasting::config::StreamingServiceConfig;
use crate::AudioState;
use tauri::State;

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
    state: State<'_, AudioState>,
) -> Result<String, String> {
    use crate::audio::broadcasting::icecast_source::{AudioCodec, AudioFormat};
    use crate::audio::broadcasting::service::{
        connect_streaming_to_mixer, initialize_streaming, StreamingServiceConfig,
    };

    println!(
        "🔧 Initializing Icecast streaming: {}:{}{}",
        server_host, server_port, mount_point
    );

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
            sample_rate: crate::types::DEFAULT_SAMPLE_RATE,
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

    Ok(format!(
        "Icecast streaming initialized: {}:{}{}",
        server_host, server_port, mount_point
    ))
}

#[tauri::command]
pub async fn start_icecast_streaming(
    audio_state: State<'_, AudioState>,
    config: StreamingServiceConfig,
) -> Result<String, String> {
    println!(
        "🎯 Starting Icecast streaming with config to {}:{}{}",
        config.server_host, config.server_port, config.mount_point
    );

    // Step 1: Send command to IsolatedAudioManager to create Icecast OutputWorker
    let stream_id = uuid::Uuid::new_v4().to_string();
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    let command = crate::audio::mixer::stream_management::AudioCommand::StartIcecast {
        stream_id: stream_id.clone(),
        config: config.clone(),
        response_tx,
    };

    if let Err(e) = audio_state.audio_command_tx.send(command).await {
        return Err(format!("Failed to send Icecast start command: {}", e));
    }

    // Step 2: Wait for the RTRB consumer from the audio pipeline
    let icecast_consumer = match response_rx.await {
        Ok(Ok(consumer)) => consumer,
        Ok(Err(e)) => return Err(format!("Failed to create Icecast output worker: {}", e)),
        Err(e) => return Err(format!("Failed to receive response: {}", e)),
    };

    println!("🔄 Received RTRB consumer from audio pipeline");

    // Step 3: Start Icecast streaming service with the RTRB consumer
    use crate::audio::broadcasting::service::start_streaming_with_consumer;

    match start_streaming_with_consumer(config, icecast_consumer).await {
        Ok(()) => {
            println!("✅ Icecast streaming started with stream ID: {}", stream_id);
            Ok(format!(
                "Streaming started successfully with stream ID: {}",
                stream_id
            ))
        }
        Err(e) => {
            println!("❌ Failed to start Icecast streaming service: {}", e);

            // **CLEANUP**: Icecast service failed, need to clean up the OutputWorker
            println!("🧹 Cleaning up OutputWorker after Icecast service failure...");
            let (cleanup_tx, cleanup_rx) = tokio::sync::oneshot::channel();
            let cleanup_command =
                crate::audio::mixer::stream_management::AudioCommand::StopIcecast {
                    stream_id: stream_id.clone(),
                    response_tx: cleanup_tx,
                };

            if let Err(cleanup_err) = audio_state.audio_command_tx.send(cleanup_command).await {
                println!("⚠️ Failed to send cleanup command: {}", cleanup_err);
            } else {
                match cleanup_rx.await {
                    Ok(Ok(())) => println!("✅ OutputWorker cleaned up successfully"),
                    Ok(Err(cleanup_err)) => {
                        println!("⚠️ Failed to cleanup OutputWorker: {}", cleanup_err)
                    }
                    Err(cleanup_err) => {
                        println!("⚠️ Failed to receive cleanup response: {}", cleanup_err)
                    }
                }
            }

            Err(format!("Failed to start streaming: {}", e))
        }
    }
}

#[tauri::command]
pub async fn stop_icecast_streaming(
    audio_state: State<'_, AudioState>,
    stream_id: String,
) -> Result<String, String> {
    println!("🛑 Stopping Icecast streaming for stream: {}", stream_id);

    // Step 1: Stop the Icecast streaming service first
    use crate::audio::broadcasting::service::stop_streaming;

    match stop_streaming().await {
        Ok(()) => println!("✅ Icecast streaming service stopped"),
        Err(e) => {
            println!(
                "⚠️ Error stopping Icecast service (continuing with cleanup): {}",
                e
            );
        }
    }

    // Step 2: Send command to IsolatedAudioManager to remove Icecast OutputWorker
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    let command = crate::audio::mixer::stream_management::AudioCommand::StopIcecast {
        stream_id: stream_id.clone(),
        response_tx,
    };

    if let Err(e) = audio_state.audio_command_tx.send(command).await {
        return Err(format!("Failed to send Icecast stop command: {}", e));
    }

    // Step 3: Wait for confirmation that OutputWorker is cleaned up
    match response_rx.await {
        Ok(Ok(())) => {
            println!(
                "✅ Icecast streaming stopped completely for stream: {}",
                stream_id
            );
            Ok(format!(
                "Streaming stopped successfully for stream: {}",
                stream_id
            ))
        }
        Ok(Err(e)) => {
            println!("❌ Failed to stop Icecast output worker: {}", e);
            Err(format!("Failed to stop streaming: {}", e))
        }
        Err(e) => {
            println!("❌ Failed to receive stop response: {}", e);
            Err(format!("Failed to receive stop confirmation: {}", e))
        }
    }
}

#[tauri::command]
pub async fn update_icecast_metadata(title: String, artist: String) -> Result<String, String> {
    use crate::audio::broadcasting::service::update_stream_metadata;

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
    use crate::audio::broadcasting::service::get_streaming_status;

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
    use crate::audio::broadcasting::service::set_stream_bitrate;

    println!("🎵 Setting stream bitrate to {}kbps", bitrate);

    match set_stream_bitrate(bitrate).await {
        Ok(()) => {
            println!(
                "✅ Bitrate set to {}kbps (restart streaming to apply)",
                bitrate
            );
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
    use crate::audio::broadcasting::service::get_available_bitrates;

    let bitrates = get_available_bitrates().await;
    Ok(bitrates)
}

#[tauri::command]
pub async fn get_current_stream_bitrate() -> Result<u32, String> {
    use crate::audio::broadcasting::service::get_current_stream_bitrate;

    let bitrate = get_current_stream_bitrate().await;
    Ok(bitrate)
}

#[tauri::command]
pub async fn set_variable_bitrate_streaming(enabled: bool, quality: u8) -> Result<String, String> {
    use crate::audio::broadcasting::service::set_variable_bitrate_streaming;

    println!(
        "🎵 Setting variable bitrate: enabled={}, quality=V{}",
        enabled, quality
    );

    match set_variable_bitrate_streaming(enabled, quality).await {
        Ok(()) => {
            println!(
                "✅ Variable bitrate set: enabled={}, quality=V{}",
                enabled, quality
            );
            Ok(format!(
                "Variable bitrate set: enabled={}, quality=V{}",
                enabled, quality
            ))
        }
        Err(e) => {
            eprintln!("❌ Failed to set variable bitrate: {}", e);
            Err(format!("Failed to set variable bitrate: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_variable_bitrate_settings() -> Result<(bool, u8), String> {
    use crate::audio::broadcasting::service::get_variable_bitrate_settings;

    let (enabled, quality) = get_variable_bitrate_settings().await;
    Ok((enabled, quality))
}
