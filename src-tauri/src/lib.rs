pub mod audio;
pub mod db;
pub mod entities;
pub mod log;
pub mod process_metrics;
pub mod types;

#[cfg(target_os = "macos")]
pub mod permissions;

use audio::broadcasting::StreamManager;
use audio::recording::RecordingService;
use audio::ApplicationAudioManager;

// Import command modules
pub mod commands;

// Re-export audio types for testing and external use
pub use audio::{
    get_device_monitoring_stats as get_monitoring_stats_impl, AudioChannel, AudioConfigFactory,
    AudioDatabase, AudioDeviceInfo, AudioDeviceManager, AudioMetrics, Compressor,
    DeviceMonitorStats, EQBand, FilePlayerService, Limiter, MasterVULevelEvent, MixerConfig,
    PeakDetector, RmsDetector, ThreeBandEqualizer, VULevelEvent, VirtualMixer,
};
// Re-export application audio types
pub use audio::tap::{ApplicationAudioError, ProcessInfo, TapStats};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
use tokio::sync::Mutex as AsyncMutex;
use tracing_subscriber::prelude::*;
// Removed unused import

// Import all command modules
use commands::application_audio::*;
use commands::audio_devices::*;
use commands::audio_effects::*;
use commands::audio_effects_default::*;
use commands::buses::*;
use commands::cast_configurations::*;
use commands::configurations::*;
use commands::debug::*;
use commands::file_player::*;
use commands::icecast::*;
use commands::mixer::*;
use commands::now_playing::*;
use commands::patch_colors::*;
use commands::patch_layouts::*;
use commands::process_metrics::*;
use commands::recording::*;
use commands::streaming::*;
use commands::system_audio::*;
use commands::vu_channels::*;

// File player state for managing multiple file players
use commands::file_player::FilePlayerState;

// Global state management
struct AudioState {
    device_manager: Arc<AsyncMutex<AudioDeviceManager>>,
    mixer: Arc<AsyncMutex<Option<VirtualMixer>>>,
    database: Arc<AudioDatabase>,
    audio_command_tx:
        tokio::sync::mpsc::Sender<crate::audio::mixer::stream_management::AudioCommand>,
    app_audio_manager: Arc<AsyncMutex<ApplicationAudioManager>>,
    /// Shared with `FilePlayerState`, so a player created through the commands
    /// is the same one the attach path finds when it is patched into a channel.
    file_player_manager: Arc<audio::FilePlayerManager>,
    #[cfg(target_os = "macos")]
    system_audio_router: Arc<AsyncMutex<audio::devices::SystemAudioRouter>>,
}
struct RecordingState {
    service: Arc<RecordingService>,
}
struct ApplicationAudioState {
    manager: Arc<AsyncMutex<ApplicationAudioManager>>,
}

// Initialize logging to output to both console and macOS Console.app
fn init_logging() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    // Create a formatting layer for console output
    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(false) // Hide module paths (e.g., sweet_beats_studio::audio::mixer::pipeline::output_worker) for cleaner logs
        .with_file(false)
        .with_line_number(false)
        .with_level(true)
        .with_thread_ids(false)
        .with_thread_names(false);

    // On macOS, create a simple layer that forwards to os_log via println!
    // This is a simpler approach that will show up in Console.app
    #[cfg(target_os = "macos")]
    {
        // Use the env_logger-style initialization but customize it for our needs
        tracing_subscriber::registry()
            .with(console_layer)
            .with(tracing_subscriber::filter::LevelFilter::INFO)
            .init();

        // Also set up a simple forwarding to system logger
        // macOS will automatically capture stdout/stderr from GUI apps and show them in Console.app
        // under the app's bundle identifier
        println!("🚀 SweetBeatsStudio logging initialized - logs will appear in Console.app under 'com.SweetBeatsStudio.app'");
    }

    #[cfg(not(target_os = "macos"))]
    {
        tracing_subscriber::registry()
            .with(console_layer)
            .with(tracing_subscriber::filter::LevelFilter::INFO)
            .init();
    }

    tracing::info!("🚀 SweetBeatsStudio logging system ready");
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

/// Restore system audio to previous default on panic
/// This function is called from the panic hook and uses a fresh database connection
#[cfg(target_os = "macos")]
async fn restore_system_audio_on_panic() -> Result<(), Box<dyn std::error::Error>> {
    use audio::devices::SystemAudioRouter;

    // Get database path (same logic as in main initialization)
    let database_path = dirs::home_dir()
        .map(|home| home.join(".sweet_beats_studio").join("data"))
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("data")
        })
        .join("sweet_beats_studio.db");

    // Create a fresh database connection
    let database = AudioDatabase::new(&database_path).await?;

    // Create a router and restore
    let mut router = SystemAudioRouter::new(database.sea_orm().clone());
    router.restore_original_default().await?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging system that sends logs to macOS Console.app
    init_logging();

    // Enable console logging for debugging signed app
    #[cfg(debug_assertions)]
    println!("🐛 DEBUG: Console logging enabled for signed app");

    // Set up panic hook to restore system audio before crash
    #[cfg(target_os = "macos")]
    {
        let default_panic = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            eprintln!("💥 PANIC DETECTED - Attempting to restore system audio before crash");

            // Try to restore system audio using a blocking runtime
            // This is safe in a panic scenario since we're crashing anyway
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    match restore_system_audio_on_panic().await {
                        Ok(_) => eprintln!("✅ System audio restored successfully before crash"),
                        Err(e) => {
                            eprintln!("❌ Failed to restore system audio before crash: {}", e)
                        }
                    }
                });
            } else {
                eprintln!("❌ Failed to create runtime for system audio restoration");
            }

            // Call the default panic handler
            default_panic(info);
        }));
    }

    // Initialize the Tokio runtime for database initialization
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    // Built before AudioState so both it and the file player commands work
    // through the same manager: a player made by one is found by the other.
    let file_player_service = FilePlayerService::new();
    let file_player_manager = file_player_service.get_manager();

    let audio_state = rt.block_on(async {
        // Initialize audio system
        let audio_device_manager = match AudioDeviceManager::new() {
            Ok(manager) => Arc::new(AsyncMutex::new(manager)),
            Err(e) => {
                eprintln!("Failed to initialize audio device manager: {}", e);
                std::process::exit(1);
            }
        };

        // Initialize SQLite database in user's home directory for app bundle compatibility
        let database_path = dirs::home_dir()
            .map(|home| home.join(".sweet_beats_studio").join("data"))
            .unwrap_or_else(|| {
                // Fallback to current directory for development
                std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join("data")
            })
            .join("sweet_beats_studio.db");

        tracing::info!("🗄️ Initializing database at: {}", database_path.display());

        let database = match AudioDatabase::new(&database_path).await {
            Ok(db) => Arc::new(db),
            Err(e) => {
                eprintln!(
                    "🚫 Failed to initialize database at {}",
                    database_path.display()
                );
                eprintln!("💥 Error: {}", e);

                // Print the full error chain for maximum detail
                let mut source = e.source();
                let mut level = 1;
                while let Some(err) = source {
                    eprintln!("  {}. Caused by: {}", level, err);
                    source = err.source();
                    level += 1;
                }

                eprintln!("🔧 Troubleshooting tips:");
                eprintln!(
                    "  - Check database file permissions at: {}",
                    database_path.display()
                );
                eprintln!("  - Verify migration files in src-tauri/migrations/ are valid SQL");
                eprintln!("  - Ensure no other process is using the database file");

                std::process::exit(1);
            }
        };

        tracing::info!("✅ Audio system initialization complete");

        // Create command channel for isolated audio thread communication
        let (audio_command_tx, audio_command_rx) =
            tokio::sync::mpsc::channel::<crate::audio::mixer::stream_management::AudioCommand>(100);

        // Clone database for IsolatedAudioManager thread
        let database_for_audio = database.clone();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!(
                        "❌ Failed to create runtime for IsolatedAudioManager: {}",
                        e
                    );
                    return;
                }
            };

            rt.block_on(async move {
                tracing::info!("🎵 Starting IsolatedAudioManager in dedicated thread");
                match crate::audio::mixer::stream_management::IsolatedAudioManager::new(
                    audio_command_rx,
                    Some(database_for_audio), // Pass database for channel number queries
                )
                .await
                {
                    Ok(mut isolated_audio_manager) => {
                        isolated_audio_manager.run().await;
                    }
                    Err(e) => {
                        tracing::error!("Failed to create IsolatedAudioManager: {}", e);
                    }
                }
            });
        });

        tracing::info!("🎵 IsolatedAudioManager started in dedicated thread");

        // Initialize application audio manager (shared between AudioState and ApplicationAudioState)
        let app_audio_manager_shared = Arc::new(AsyncMutex::new(ApplicationAudioManager::new()));

        #[cfg(target_os = "macos")]
        let system_audio_router = {
            use audio::devices::SystemAudioRouter;
            let router = SystemAudioRouter::new(database.sea_orm().clone());
            Arc::new(AsyncMutex::new(router))
        };

        AudioState {
            device_manager: audio_device_manager,
            mixer: Arc::new(AsyncMutex::new(None)),
            database,
            audio_command_tx,
            app_audio_manager: app_audio_manager_shared.clone(),
            file_player_manager: file_player_manager.clone(),
            #[cfg(target_os = "macos")]
            system_audio_router,
        }
    });

    // Initialize recording service
    let recording_state = RecordingState {
        service: Arc::new(RecordingService::new()),
    };

    // Shares its manager with AudioState, which is what lets a player created
    // here be found by the attach path when it is patched into a channel.
    let file_player_state = FilePlayerState {
        service: file_player_service,
    };

    // Initialize application audio state using the same app audio manager from AudioState
    let application_audio_state = ApplicationAudioState {
        manager: audio_state.app_audio_manager.clone(),
    };

    // Set up signal handlers for graceful shutdown (SIGTERM, SIGINT)
    #[cfg(target_os = "macos")]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let router_for_signals = audio_state.system_audio_router.clone();
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("❌ Failed to create runtime for signal handlers: {}", e);
                    return;
                }
            };

            rt.block_on(async {
                let mut sigterm = match signal(SignalKind::terminate()) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("❌ Failed to set up SIGTERM handler: {}", e);
                        return;
                    }
                };
                let mut sigint = match signal(SignalKind::interrupt()) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("❌ Failed to set up SIGINT handler: {}", e);
                        return;
                    }
                };

                tokio::select! {
                    _ = sigterm.recv() => {
                        tracing::info!("📡 SIGTERM received - Gracefully shutting down...");
                    }
                    _ = sigint.recv() => {
                        tracing::info!("📡 SIGINT received - Gracefully shutting down...");
                    }
                }

                // Restore system audio before exiting
                let mut router = router_for_signals.lock().await;
                if let Err(e) = router.restore_original_default().await {
                    eprintln!("❌ Failed to restore system audio on signal: {}", e);
                } else {
                    tracing::info!("✅ System audio restored successfully on signal");
                }

                std::process::exit(0);
            });
        });

        tracing::info!("📡 Signal handlers (SIGTERM, SIGINT) initialized");
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(audio_state)
        .manage(recording_state)
        .manage(file_player_state)
        .manage(application_audio_state)
        .manage(commands::now_playing::NowPlayingState::new())
        .manage(ProcessMonitorState(
            crate::process_metrics::ProcessMonitor::new(),
        ));

    // Exposes the webview to `tauri-wd` over a local HTTP server so the app can
    // be driven from WebDriver clients. Debug builds only - a release build has
    // no automation server at all.
    #[cfg(debug_assertions)]
    let builder = builder.plugin(tauri_plugin_webdriver_automation::init());

    #[cfg(target_os = "macos")]
    let builder = builder.on_window_event(|window, event| {
        if let tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed = event {
            let app_handle = window.app_handle();

            // The now-playing adapter is a child process rather than a thread,
            // so it survives the app unless something explicitly ends it.
            if let Some(now_playing) =
                app_handle.try_state::<commands::now_playing::NowPlayingState>()
            {
                let watcher = now_playing.watcher.clone();
                tauri::async_runtime::spawn(async move {
                    watcher.lock().await.stop();
                });
            }

            if let Some(audio_state) = app_handle.try_state::<AudioState>() {
                let router = audio_state.system_audio_router.clone();
                tauri::async_runtime::spawn(async move {
                    let mut router = router.lock().await;
                    if let Err(e) = router.restore_original_default().await {
                        tracing::error!("Failed to restore system audio on app close: {}", e);
                    } else {
                        tracing::info!("✅ System audio restored to original default");
                    }
                });
            }
        }
    });

    // Device hotplug tracking belongs to the app rather than the window: a
    // disconnect has to be noticed, and a reconnect recovered, whether or not
    // the UI is mounted to ask about it.
    #[cfg(target_os = "macos")]
    let builder = builder.setup(|app| {
        let audio_state = app.state::<AudioState>();
        match audio::devices::DeviceWatcher::start(
            app.handle().clone(),
            audio_state.device_manager.clone(),
            audio_state.audio_command_tx.clone(),
        ) {
            Ok(watcher) => {
                app.manage(watcher);
            }
            Err(error) => {
                tracing::error!("Failed to start Core Audio device watcher: {}", error);
            }
        }
        Ok(())
    });

    builder
        .invoke_handler(tauri::generate_handler![
            // Application lifecycle commands
            commands::app::quit_application,
            // Streaming commands
            get_listener_stats,
            // Audio device commands
            enumerate_audio_devices,
            refresh_audio_devices,
            get_audio_device,
            safe_switch_input_device,
            safe_switch_output_device,
            get_device_health,
            get_all_device_health,
            report_device_error,
            remove_input_stream,
            remove_output_stream,
            clear_session_devices,
            set_output_stream,
            start_device_monitoring,
            get_device_monitoring_stats,
            // Now-playing metadata commands
            get_now_playing,
            start_now_playing_watch,
            stop_now_playing_watch,
            is_now_playing_watch_running,
            // System audio commands
            enable_system_audio_capture,
            disable_system_audio_capture,
            get_system_audio_status,
            // Audio effects commands
            update_channel_eq,
            update_channel_compressor,
            update_channel_limiter,
            add_channel_effect,
            remove_channel_effect,
            get_channel_effects,
            get_dj_mixer_config,
            rename_mixer_channel,
            update_master_gain,
            // Bus routing commands
            list_audio_buses,
            restore_audio_buses,
            set_output_sources,
            // Cast configurations
            list_cast_configurations,
            create_cast_configuration,
            update_cast_configuration,
            delete_cast_configuration,
            set_cast_configuration_password,
            start_cast,
            list_cast_targets,
            add_cast_target,
            remove_cast_target,
            list_patch_colors,
            set_patch_color,
            clear_patch_color,
            list_patch_layouts,
            set_patch_layout,
            clear_patch_layout,
            clear_patch_layouts,
            create_audio_bus,
            remove_audio_bus,
            set_audio_bus_gain,
            set_input_bus_sends,
            set_output_audio_bus,
            get_pipeline_latency,
            get_process_metrics,
            set_debug_log_config,
            get_debug_log_config,
            get_output_health,
            // Icecast commands
            start_icecast_streaming,
            stop_icecast_streaming,
            update_icecast_metadata,
            get_icecast_streaming_status,
            set_stream_bitrate,
            get_available_stream_bitrates,
            get_current_stream_bitrate,
            set_variable_bitrate_streaming,
            get_variable_bitrate_settings,
            // Recording commands
            start_recording,
            stop_recording,
            get_recording_status,
            save_recording_config,
            get_recording_configs,
            get_recording_history,
            create_default_recording_config,
            select_recording_directory,
            // File player commands
            create_file_player,
            remove_file_player,
            restore_file_players,
            list_file_players,
            get_file_player_devices,
            // Queues as a collection, apart from whichever patch is loaded
            list_queues,
            queue_tracks,
            queue_plays,
            clear_queue_plays,
            rename_queue,
            list_queue_targets,
            add_queue_target,
            remove_queue_target,
            add_track_to_player,
            remove_track_from_player,
            move_track_in_player,
            set_player_breakpoint,
            get_player_queue,
            clear_player_queue,
            control_file_player,
            get_player_status,
            browse_audio_files,
            get_supported_audio_formats,
            validate_audio_file,
            // Application audio commands
            get_known_audio_applications,
            get_all_available_applications,
            add_audio_application,
            update_audio_application_name,
            remove_audio_application,
            stop_application_audio_capture,
            get_active_audio_captures,
            stop_all_audio_captures,
            get_application_info,
            refresh_audio_applications,
            request_audio_capture_permissions,
            // Tap lifecycle management commands
            get_tap_statistics,
            cleanup_stale_taps,
            shutdown_application_audio_manager,
            // Recording commands
            start_recording,
            stop_recording,
            get_recording_status,
            save_recording_config,
            get_recording_configs,
            get_recording_history,
            create_default_recording_config,
            select_recording_directory,
            get_metadata_presets,
            get_recording_presets,
            update_recording_metadata,
            // Configuration commands
            get_reusable_configurations,
            get_active_session_configuration,
            create_session_from_reusable,
            save_session_to_reusable,
            save_session_as_new_reusable,
            get_configuration_by_id,
            create_reusable_configuration,
            get_configured_audio_devices_by_config,
            // Audio effects default commands
            get_audio_effects_defaults,
            update_audio_effects_default_gain,
            update_audio_effects_default_pan,
            update_audio_effects_default_effects_enabled,
            update_audio_effects_default_mute,
            update_audio_effects_default_solo,
            // VU Events commands
            initialize_vu_channels
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
