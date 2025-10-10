pub mod audio;
pub mod db;
pub mod entities;
pub mod log;
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
use commands::configurations::*;
use commands::debug::*;
use commands::file_player::*;
use commands::icecast::*;
use commands::mixer::*;
use commands::recording::*;
use commands::streaming::*;
use commands::system_audio::*;
use commands::vu_channels::*;

// File player state for managing multiple file players
use commands::file_player::FilePlayerState;

// Global state management
struct StreamState(Mutex<Option<StreamManager>>);
struct AudioState {
    device_manager: Arc<AsyncMutex<AudioDeviceManager>>,
    mixer: Arc<AsyncMutex<Option<VirtualMixer>>>,
    database: Arc<AudioDatabase>,
    audio_command_tx:
        tokio::sync::mpsc::Sender<crate::audio::mixer::stream_management::AudioCommand>,
    app_audio_manager: Arc<AsyncMutex<ApplicationAudioManager>>,
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
        .with_target(false) // Hide module paths (e.g., sendin_beats_lib::audio::mixer::pipeline::output_worker) for cleaner logs
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
        println!("🚀 SendinBeats logging initialized - logs will appear in Console.app under 'com.sendinbeats.app'");
    }

    #[cfg(not(target_os = "macos"))]
    {
        tracing_subscriber::registry()
            .with(console_layer)
            .with(tracing_subscriber::filter::LevelFilter::INFO)
            .init();
    }

    tracing::info!("🚀 SendinBeats logging system ready");
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging system that sends logs to macOS Console.app
    init_logging();

    // Enable console logging for debugging signed app
    #[cfg(debug_assertions)]
    println!("🐛 DEBUG: Console logging enabled for signed app");

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

        // Initialize SQLite database in user's home directory for app bundle compatibility
        let database_path = dirs::home_dir()
            .map(|home| home.join(".sendin_beats").join("data"))
            .unwrap_or_else(|| {
                // Fallback to current directory for development
                std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join("data")
            })
            .join("sendin_beats.db");

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
            #[cfg(target_os = "macos")]
            system_audio_router,
        }
    });

    // Initialize recording service
    let recording_state = RecordingState {
        service: Arc::new(RecordingService::new()),
    };

    // Initialize file player service
    let file_player_state = FilePlayerState {
        service: FilePlayerService::new(),
    };

    // Initialize application audio state using the same app audio manager from AudioState
    let application_audio_state = ApplicationAudioState {
        manager: audio_state.app_audio_manager.clone(),
    };

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(StreamState(Mutex::new(None)))
        .manage(audio_state)
        .manage(recording_state)
        .manage(file_player_state)
        .manage(application_audio_state);

    #[cfg(target_os = "macos")]
    let builder = builder.on_window_event(|window, event| {
        if let tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed = event {
            let app_handle = window.app_handle();
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

    builder
        .invoke_handler(tauri::generate_handler![
            // Streaming commands
            connect_to_stream,
            disconnect_from_stream,
            start_streaming,
            stop_streaming,
            update_metadata,
            get_stream_status,
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
            set_output_stream,
            start_device_monitoring,
            get_device_monitoring_stats,
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
            update_master_gain,
            set_debug_log_config,
            get_debug_log_config,
            // Icecast commands
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
            list_file_players,
            get_file_player_devices,
            add_track_to_player,
            remove_track_from_player,
            get_player_queue,
            clear_player_queue,
            control_file_player,
            get_player_status,
            browse_audio_files,
            get_supported_audio_formats,
            validate_audio_file,
            // Application audio commands
            get_known_audio_applications,
            stop_application_audio_capture,
            get_active_audio_captures,
            stop_all_audio_captures,
            get_application_info,
            refresh_audio_applications,
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
            update_audio_effects_default_mute,
            update_audio_effects_default_solo,
            // VU Events commands
            initialize_vu_channels
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
