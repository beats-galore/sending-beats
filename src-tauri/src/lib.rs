pub mod audio;
pub mod db;
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
    AudioDatabase, AudioDeviceInfo, AudioDeviceManager, AudioEventBus, AudioMetrics, ChannelConfig,
    Compressor, DeviceMonitorStats, EQBand, FilePlayerService, Limiter, MixerConfig, PeakDetector,
    RmsDetector, ThreeBandEqualizer, VULevelData, VirtualMixer,
};
// Re-export application audio types
pub use audio::tap::{ApplicationAudioError, ProcessInfo, TapStats};
use std::sync::{Arc, Mutex};
use tauri::State;
use tokio::sync::Mutex as AsyncMutex;
use tracing_subscriber::prelude::*;
// Removed unused import

// Import all command modules
use commands::application_audio::*;
use commands::audio_devices::*;
use commands::audio_effects::*;
use commands::debug::*;
use commands::file_player::*;
use commands::icecast::*;
use commands::mixer::*;
use commands::recording::*;
use commands::streaming::*;

// File player state for managing multiple file players
use commands::file_player::FilePlayerState;

// Global state management
struct StreamState(Mutex<Option<StreamManager>>);
struct AudioState {
    device_manager: Arc<AsyncMutex<AudioDeviceManager>>,
    mixer: Arc<AsyncMutex<Option<VirtualMixer>>>,
    database: Arc<AudioDatabase>,
    event_bus: Arc<AudioEventBus>,
    audio_command_tx:
        tokio::sync::mpsc::Sender<crate::audio::mixer::stream_management::AudioCommand>,
}
struct RecordingState {
    service: Arc<RecordingService>,
}
struct ApplicationAudioState {
    manager: Arc<ApplicationAudioManager>,
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

        // Initialize event bus for lock-free audio data transfer
        let event_bus = Arc::new(AudioEventBus::new(1000)); // Buffer up to 1000 events

        tracing::info!("✅ Audio system initialization complete");

        // Create command channel for isolated audio thread communication
        let (audio_command_tx, audio_command_rx) =
            tokio::sync::mpsc::channel::<crate::audio::mixer::stream_management::AudioCommand>(100);

        // Start IsolatedAudioManager in a dedicated thread with its own runtime
        // This avoids Send+Sync issues with CPAL streams on macOS
        std::thread::spawn(move || {
            // Create a new runtime for this thread since we can't send the runtime
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

        AudioState {
            device_manager: audio_device_manager,
            mixer: Arc::new(AsyncMutex::new(None)),
            database,
            event_bus,
            audio_command_tx,
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

    // Initialize application audio manager
    let application_audio_state = ApplicationAudioState {
        manager: Arc::new(ApplicationAudioManager::new()),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(StreamState(Mutex::new(None)))
        .manage(audio_state)
        .manage(recording_state)
        .manage(file_player_state)
        .manage(application_audio_state)
        .invoke_handler(tauri::generate_handler![
            // Streaming commands
            greet,
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
            add_input_stream,
            remove_input_stream,
            set_output_stream,
            start_device_monitoring,
            get_device_monitoring_stats,
            // Mixer commands
            add_output_device,
            remove_output_device,
            update_output_device,
            // CoreAudio specific commands
            enumerate_coreaudio_devices,
            get_device_type_info,
            is_coreaudio_device,
            // Audio effects commands
            update_channel_eq,
            update_channel_compressor,
            update_channel_limiter,
            add_channel_effect,
            remove_channel_effect,
            get_channel_effects,
            get_dj_mixer_config,
            // Debug commands
            // get_recent_vu_levels,     // TODO: Implement with new schema
            // get_recent_master_levels, // TODO: Implement with new schema
            // save_channel_config,      // TODO: Implement with new schema
            // load_channel_configs,     // TODO: Implement with new schema
            cleanup_old_levels,
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
            get_available_audio_applications,
            get_known_audio_applications,
            start_application_audio_capture,
            stop_application_audio_capture,
            get_active_audio_captures,
            stop_all_audio_captures,
            get_application_info,
            refresh_audio_applications,
            create_mixer_input_for_application,
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
            update_recording_metadata
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
