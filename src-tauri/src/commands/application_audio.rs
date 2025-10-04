use crate::audio::tap::{ApplicationAudioError, ProcessInfo, TapStats};
use crate::ApplicationAudioState;
use tauri::State;

// ================================================================================================
// APPLICATION AUDIO COMMANDS
// ================================================================================================

#[tauri::command]
pub async fn get_known_audio_applications(
    app_audio_state: State<'_, ApplicationAudioState>,
) -> Result<Vec<ProcessInfo>, String> {
    println!("🎵 Getting known audio applications...");

    let manager = app_audio_state.manager.lock().await;
    match manager.get_available_applications().await {
        Ok(all_apps) => {
            println!(
                "🔍 Processing {} total apps for known app filtering",
                all_apps.len()
            );

            // Filter for apps with bundle IDs (known apps)
            let known_apps: Vec<ProcessInfo> = all_apps
                .into_iter()
                .filter(|app| {
                    let has_bundle = app.bundle_id.is_some();
                    if has_bundle {
                        println!(
                            "✅ Known app found: {} [{}]",
                            app.name,
                            app.bundle_id.as_ref().unwrap()
                        );
                    }
                    has_bundle
                })
                .collect();

            println!("✅ Found {} known audio applications", known_apps.len());
            Ok(known_apps)
        }
        Err(e) => {
            eprintln!("❌ Failed to get known audio applications: {}", e);
            Err(format!("Failed to get known audio applications: {}", e))
        }
    }
}

#[tauri::command]
pub async fn stop_application_audio_capture(
    app_audio_state: State<'_, ApplicationAudioState>,
    pid: u32,
) -> Result<String, String> {
    println!("🛑 Stopping audio capture for PID: {}", pid);

    let manager = app_audio_state.manager.lock().await;
    match manager.stop_capturing_app(pid).await {
        Ok(()) => {
            println!("✅ Stopped audio capture for PID: {}", pid);
            Ok(format!(
                "Successfully stopped capturing audio from application (PID: {})",
                pid
            ))
        }
        Err(e) => {
            eprintln!("❌ Failed to stop audio capture for PID {}: {}", pid, e);
            Err(format!("Failed to stop audio capture: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_active_audio_captures(
    app_audio_state: State<'_, ApplicationAudioState>,
) -> Result<Vec<ProcessInfo>, String> {
    println!("📊 Getting active audio captures...");

    let manager = app_audio_state.manager.lock().await;
    let active_captures = manager.get_active_captures().await;

    println!("✅ Found {} active audio captures", active_captures.len());
    for capture in &active_captures {
        println!("  - {} (PID: {})", capture.name, capture.pid);
    }

    Ok(active_captures)
}

#[tauri::command]
pub async fn stop_all_audio_captures(
    app_audio_state: State<'_, ApplicationAudioState>,
) -> Result<String, String> {
    println!("🛑 Stopping all audio captures...");

    let manager = app_audio_state.manager.lock().await;
    match manager.stop_all_captures().await {
        Ok(()) => {
            println!("✅ Stopped all audio captures");
            Ok("Successfully stopped all audio captures".to_string())
        }
        Err(e) => {
            eprintln!("❌ Failed to stop all audio captures: {}", e);
            Err(format!("Failed to stop all audio captures: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_application_info(
    app_audio_state: State<'_, ApplicationAudioState>,
    pid: u32,
) -> Result<Option<ProcessInfo>, String> {
    println!("ℹ️ Getting application info for PID: {}", pid);

    let manager = app_audio_state.manager.lock().await;
    match manager.get_available_applications().await {
        Ok(apps) => {
            let app_info = apps.into_iter().find(|app| app.pid == pid);

            if let Some(ref info) = app_info {
                println!(
                    "✅ Found application: {} ({})",
                    info.name,
                    info.bundle_id.as_ref().unwrap_or(&"unknown".to_string())
                );
            } else {
                println!("⚠️ Application not found for PID: {}", pid);
            }

            Ok(app_info)
        }
        Err(e) => {
            eprintln!("❌ Failed to get application info: {}", e);
            Err(format!("Failed to get application info: {}", e))
        }
    }
}

#[tauri::command]
pub async fn refresh_audio_applications(
    app_audio_state: State<'_, ApplicationAudioState>,
) -> Result<Vec<ProcessInfo>, String> {
    println!("🔄 Refreshing audio applications list...");

    // This will force a new scan by calling get_available_applications
    get_known_audio_applications(app_audio_state).await
}

// ================================================================================================
// TAP LIFECYCLE MANAGEMENT COMMANDS
// ================================================================================================

#[tauri::command]
pub async fn get_tap_statistics(
    app_audio_state: State<'_, ApplicationAudioState>,
) -> Result<Vec<TapStats>, String> {
    println!("📊 Getting tap statistics...");

    let manager = app_audio_state.manager.lock().await;
    let stats = manager.get_tap_stats().await;

    println!("✅ Retrieved statistics for {} active taps", stats.len());
    for stat in &stats {
        println!(
            "  - {} (PID: {}) - Age: {:?}, Errors: {}, Active: {}, Alive: {}",
            stat.process_name,
            stat.pid,
            stat.age,
            stat.error_count,
            stat.is_capturing,
            stat.process_alive
        );
    }

    Ok(stats)
}

#[tauri::command]
pub async fn cleanup_stale_taps(
    app_audio_state: State<'_, ApplicationAudioState>,
) -> Result<usize, String> {
    println!("🧹 Performing manual cleanup of stale taps...");

    let manager = app_audio_state.manager.lock().await;
    match manager.cleanup_stale_taps().await {
        Ok(cleaned_count) => {
            println!("✅ Cleaned up {} stale taps", cleaned_count);
            Ok(cleaned_count)
        }
        Err(e) => {
            eprintln!("❌ Failed to cleanup stale taps: {}", e);
            Err(format!("Failed to cleanup stale taps: {}", e))
        }
    }
}

#[tauri::command]
pub async fn shutdown_application_audio_manager(
    app_audio_state: State<'_, ApplicationAudioState>,
) -> Result<String, String> {
    println!("🛑 Shutting down Application Audio Manager...");

    let manager = app_audio_state.manager.lock().await;
    match manager.shutdown().await {
        Ok(()) => {
            println!("✅ Application Audio Manager shutdown complete");
            Ok("Application Audio Manager shutdown successfully".to_string())
        }
        Err(e) => {
            eprintln!("❌ Failed to shutdown Application Audio Manager: {}", e);
            Err(format!(
                "Failed to shutdown Application Audio Manager: {}",
                e
            ))
        }
    }
}
