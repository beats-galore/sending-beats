use crate::audio::tap::{ProcessInfo, TapStats};
use crate::ApplicationAudioState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[cfg(target_os = "macos")]
use crate::audio::screencapture::{self, discovery};

// ================================================================================================
// APPLICATION AUDIO COMMANDS
// ================================================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableApplicationInfo {
    pub pid: i32,
    pub bundle_identifier: String,
    pub application_name: String,
    pub is_in_database: bool,
}

#[tauri::command]
pub async fn get_all_available_applications(
    audio_state: State<'_, crate::AudioState>,
) -> Result<Vec<AvailableApplicationInfo>, String> {
    println!("📋 Getting all available applications from ScreenCaptureKit...");

    #[cfg(target_os = "macos")]
    {
        use crate::entities::audio_application;
        use sea_orm::EntityTrait;

        match screencapture::get_available_applications() {
            Ok(apps) => {
                let total_apps = apps.len();
                println!("🔍 Found {} applications via ScreenCaptureKit", total_apps);

                let db = audio_state.database.sea_orm();
                let known_audio_apps = audio_application::Entity::find()
                    .all(db)
                    .await
                    .map_err(|e| format!("Failed to query audio applications: {}", e))?;

                let known_bundle_ids: std::collections::HashSet<String> = known_audio_apps
                    .iter()
                    .filter(|app| app.operating_system == "macos")
                    .map(|app| app.bundle_identifier.clone())
                    .collect();

                let mut seen_bundle_ids = std::collections::HashSet::new();
                let available_apps: Vec<AvailableApplicationInfo> = apps
                    .into_iter()
                    .filter_map(|app| {
                        if app.bundle_identifier.is_empty() {
                            return None;
                        }

                        if !seen_bundle_ids.insert(app.bundle_identifier.clone()) {
                            return None;
                        }

                        Some(AvailableApplicationInfo {
                            pid: app.pid,
                            bundle_identifier: app.bundle_identifier.clone(),
                            application_name: app.application_name,
                            is_in_database: known_bundle_ids.contains(&app.bundle_identifier),
                        })
                    })
                    .collect();

                println!(
                    "✅ Returning {} unique applications ({} in database)",
                    available_apps.len(),
                    available_apps.iter().filter(|a| a.is_in_database).count()
                );
                Ok(available_apps)
            }
            Err(e) => {
                eprintln!("❌ Failed to get applications via ScreenCaptureKit: {}", e);
                Err(format!("Failed to get available applications: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Application audio capture is only supported on macOS".to_string())
    }
}

#[tauri::command]
pub async fn add_audio_application(
    audio_state: State<'_, crate::AudioState>,
    bundle_identifier: String,
    application_name: String,
    operating_system: String,
) -> Result<String, String> {
    println!(
        "➕ Adding audio application: {} ({})",
        application_name, bundle_identifier
    );

    use crate::entities::audio_application;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

    let db = audio_state.database.sea_orm();

    let existing = audio_application::Entity::find()
        .filter(audio_application::Column::ApplicationName.eq(&application_name))
        .one(db)
        .await
        .map_err(|e| format!("Failed to check for existing application: {}", e))?;

    if let Some(existing_app) = existing {
        if existing_app.bundle_identifier != bundle_identifier {
            println!(
                "📝 Updating bundle_identifier for '{}' from '{}' to '{}'",
                application_name, existing_app.bundle_identifier, bundle_identifier
            );

            let mut active_model: audio_application::ActiveModel = existing_app.into();
            active_model.bundle_identifier = Set(bundle_identifier.clone());
            active_model.updated_at = Set(chrono::Utc::now());

            active_model
                .update(db)
                .await
                .map_err(|e| format!("Failed to update application: {}", e))?;

            println!("✅ Updated bundle_identifier for '{}'", application_name);
            Ok(format!(
                "Updated bundle_identifier for application: {}",
                application_name
            ))
        } else {
            println!(
                "ℹ️ Application '{}' already exists with same bundle_identifier",
                application_name
            );
            Ok(format!("Application already exists: {}", application_name))
        }
    } else {
        let new_app = audio_application::Model::new(
            bundle_identifier,
            application_name.clone(),
            operating_system,
        );

        new_app
            .insert(db)
            .await
            .map_err(|e| format!("Failed to insert application: {}", e))?;

        println!("✅ Added new audio application: {}", application_name);
        Ok(format!(
            "Successfully added application: {}",
            application_name
        ))
    }
}

#[tauri::command]
pub async fn update_audio_application_name(
    audio_state: State<'_, crate::AudioState>,
    bundle_identifier: String,
    new_application_name: String,
) -> Result<String, String> {
    println!(
        "📝 Updating audio application name for {}: {}",
        bundle_identifier, new_application_name
    );

    use crate::entities::audio_application;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

    let db = audio_state.database.sea_orm();

    let app = audio_application::Entity::find()
        .filter(audio_application::Column::BundleIdentifier.eq(&bundle_identifier))
        .one(db)
        .await
        .map_err(|e| format!("Failed to find application: {}", e))?;

    if let Some(app_model) = app {
        let mut active_model: audio_application::ActiveModel = app_model.into();
        active_model.application_name = Set(new_application_name.clone());
        active_model.updated_at = Set(chrono::Utc::now());

        active_model
            .update(db)
            .await
            .map_err(|e| format!("Failed to update application name: {}", e))?;

        println!(
            "✅ Updated application name for {}: {}",
            bundle_identifier, new_application_name
        );
        Ok(format!(
            "Successfully updated application name: {}",
            new_application_name
        ))
    } else {
        println!("⚠️ Application not found: {}", bundle_identifier);
        Err(format!("Application not found: {}", bundle_identifier))
    }
}

#[tauri::command]
pub async fn remove_audio_application(
    audio_state: State<'_, crate::AudioState>,
    bundle_identifier: String,
) -> Result<String, String> {
    println!("🗑️ Removing audio application: {}", bundle_identifier);

    use crate::entities::audio_application;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let db = audio_state.database.sea_orm();

    let app = audio_application::Entity::find()
        .filter(audio_application::Column::BundleIdentifier.eq(&bundle_identifier))
        .one(db)
        .await
        .map_err(|e| format!("Failed to find application: {}", e))?;

    if let Some(app_model) = app {
        use sea_orm::ModelTrait;
        app_model
            .delete(db)
            .await
            .map_err(|e| format!("Failed to delete application: {}", e))?;

        println!("✅ Removed audio application: {}", bundle_identifier);
        Ok(format!(
            "Successfully removed application: {}",
            bundle_identifier
        ))
    } else {
        println!("⚠️ Application not found: {}", bundle_identifier);
        Err(format!("Application not found: {}", bundle_identifier))
    }
}

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

                // Build a map of bundle_identifier -> saved application name
                let known_apps_map: std::collections::HashMap<String, String> = known_audio_apps
                    .iter()
                    .filter(|app| app.is_enabled && app.operating_system == "macos")
                    .map(|app| (app.bundle_identifier.clone(), app.application_name.clone()))
                    .collect();

                println!(
                    "📋 Loaded {} enabled audio applications from database",
                    known_apps_map.len()
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
                        if let Some(saved_name) = known_apps_map.get(&app.bundle_identifier) {
                            Some(ProcessInfo {
                                pid,
                                name: saved_name.clone(),
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
