use crate::audio::tap::{ApplicationAudioError, ProcessInfo, TapStats};
use crate::ApplicationAudioState;
use tauri::State;

#[cfg(target_os = "macos")]
use crate::audio::screencapture::{self, discovery};

// ================================================================================================
// APPLICATION AUDIO COMMANDS
// ================================================================================================

#[tauri::command]
pub async fn get_known_audio_applications(
    app_audio_state: State<'_, ApplicationAudioState>,
    audio_state: State<'_, crate::AudioState>,
) -> Result<Vec<ProcessInfo>, String> {
    println!("🎵 Getting known audio applications (ScreenCaptureKit)...");

    #[cfg(target_os = "macos")]
    {
        use crate::entities::audio_application;
        use sea_orm::EntityTrait;

        match screencapture::get_available_applications() {
            Ok(apps) => {
                let total_apps = apps.len();
                println!(
                    "🔍 Found {} applications via ScreenCaptureKit (after Swift-side filtering)",
                    total_apps
                );

                // Query database for known audio applications
                let db = audio_state.database.sea_orm();
                let known_audio_apps = audio_application::Entity::find()
                    .all(db)
                    .await
                    .map_err(|e| format!("Failed to query audio applications: {}", e))?;

                // Build a set of known bundle identifiers that are enabled
                let known_bundle_ids: std::collections::HashSet<String> = known_audio_apps
                    .iter()
                    .filter(|app| app.is_enabled && app.operating_system == "macos")
                    .map(|app| app.bundle_identifier.clone())
                    .collect();

                println!(
                    "📋 Loaded {} enabled audio applications from database",
                    known_bundle_ids.len()
                );

                // Filter and deduplicate by PID
                let mut seen_pids = std::collections::HashSet::new();
                let process_infos: Vec<ProcessInfo> = apps
                    .into_iter()
                    .filter_map(|app| {
                        let pid = app.pid as u32;

                        // Skip duplicates
                        if !seen_pids.insert(pid) {
                            return None;
                        }

                        // Only include if in the known audio applications database
                        if known_bundle_ids.contains(&app.bundle_identifier) {
                            Some(ProcessInfo {
                                pid,
                                name: app.application_name.clone(),
                                bundle_id: Some(app.bundle_identifier),
                                icon_path: None,
                                is_audio_capable: true,
                                is_playing_audio: false,
                            })
                        } else {
                            None
                        }
                    })
                    .collect();

                println!(
                    "✅ Returning {} known audio applications (filtered from {}, deduplicated)",
                    process_infos.len(),
                    total_apps
                );
                Ok(process_infos)
            }
            Err(e) => {
                eprintln!("❌ Failed to get applications via ScreenCaptureKit: {}", e);
                Err(format!("Failed to get audio applications: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Application audio capture is only supported on macOS".to_string())
    }
}

#[tauri::command]
pub async fn check_screen_recording_permission() -> Result<bool, String> {
    println!("🔐 Checking Screen Recording permission...");

    #[cfg(target_os = "macos")]
    {
        let has_permission = discovery::check_screen_recording_permission();
        if has_permission {
            println!("✅ Screen Recording permission available");
        } else {
            println!("⚠️ Screen Recording permission not available");
        }
        Ok(has_permission)
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(false)
    }
}

#[tauri::command]
pub async fn request_audio_capture_permissions() -> Result<String, String> {
    println!("🔐 Requesting audio capture permissions...");

    #[cfg(target_os = "macos")]
    {
        let has_permission = discovery::check_screen_recording_permission();
        if has_permission {
            Ok("Screen Recording permission already granted".to_string())
        } else {
            Ok("Please grant Screen Recording permission in System Settings > Privacy & Security > Screen Recording".to_string())
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Not supported on this platform".to_string())
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
    audio_state: State<'_, crate::AudioState>,
) -> Result<Vec<ProcessInfo>, String> {
    println!("🔄 Refreshing audio applications list...");

    // This will force a new scan by calling get_available_applications
    get_known_audio_applications(app_audio_state, audio_state).await
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
