pub mod streaming;
pub mod audio;
pub mod icecast_source;
pub mod streaming_service;
pub mod recording_service;
pub mod application_audio;

#[cfg(target_os = "macos")]
pub mod coreaudio_taps;

#[cfg(target_os = "macos")]
pub mod tcc_permissions;

use streaming::{StreamManager};
use recording_service::{RecordingService};
use application_audio::{ApplicationAudioManager};

// Import command modules
pub mod commands;

// Re-export audio types for testing and external use
pub use audio::{
    AudioDeviceManager, VirtualMixer, MixerConfig, AudioDeviceInfo, AudioChannel, 
    AudioMetrics, MixerCommand, AudioConfigFactory, EQBand, ThreeBandEqualizer, 
    Compressor, Limiter, PeakDetector, RmsDetector, AudioDatabase, AudioEventBus,
    VULevelData, MasterLevelData, ChannelConfig, OutputRouteConfig, DeviceMonitorStats,
    initialize_device_monitoring, get_device_monitoring_stats as get_monitoring_stats_impl, 
    stop_device_monitoring as stop_monitoring_impl, FilePlayerService
};
use std::sync::{Arc, Mutex};
use tauri::State;
use tokio::sync::Mutex as AsyncMutex;
// Removed unused import

// Import all command modules
use commands::streaming::*;
use commands::audio_devices::*;
use commands::mixer::*;
use commands::audio_effects::*;
use commands::recording::*;
use commands::icecast::*;
use commands::debug::*;
use commands::file_player::*;
use commands::application_audio::*;

// File player state for managing multiple file players
use commands::file_player::FilePlayerState;

// Global state management
struct StreamState(Mutex<Option<StreamManager>>);
struct AudioState {
    device_manager: Arc<AsyncMutex<AudioDeviceManager>>,
    mixer: Arc<AsyncMutex<Option<VirtualMixer>>>,
    database: Arc<AudioDatabase>,
    event_bus: Arc<AudioEventBus>,
}
struct RecordingState {
    service: Arc<RecordingService>,
}
struct ApplicationAudioState {
    manager: Arc<ApplicationAudioManager>,
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
        
        // Initialize SQLite database in src-tauri directory for now
        let database_path = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("data")
            .join("sendin_beats.db");
            
        println!("🗄️  Initializing database at: {}", database_path.display());
        
        let database = match AudioDatabase::new(&database_path).await {
            Ok(db) => Arc::new(db),
            Err(e) => {
                eprintln!("Failed to initialize database: {}", e);
                std::process::exit(1);
            }
        };
        
        // Initialize event bus for lock-free audio data transfer
        let event_bus = Arc::new(AudioEventBus::new(1000)); // Buffer up to 1000 events
        
        println!("✅ Audio system initialization complete");
        
        AudioState {
            device_manager: audio_device_manager,
            mixer: Arc::new(AsyncMutex::new(None)),
            database,
            event_bus,
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
            stop_device_monitoring,
            get_device_monitoring_stats,
            // Mixer commands
            create_mixer,
            start_mixer,
            stop_mixer,
            add_mixer_channel,
            update_mixer_channel,
            get_mixer_metrics,
            get_channel_levels,
            get_master_levels,
            send_mixer_command,
            get_dj_mixer_config,
            get_streaming_mixer_config,
            add_output_device,
            remove_output_device,
            update_output_device,
            get_output_devices,
            // Audio effects commands
            update_channel_eq,
            update_channel_compressor,
            update_channel_limiter,
            add_channel_effect,
            remove_channel_effect,
            get_channel_effects,
            // Debug commands
            get_recent_vu_levels,
            get_recent_master_levels,
            save_channel_config,
            load_channel_configs,
            cleanup_old_levels,
            set_audio_debug_enabled,
            get_audio_debug_enabled,
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
            check_audio_capture_permissions,
            request_audio_capture_permissions,
            get_application_info,
            refresh_audio_applications,
            create_mixer_input_for_application
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}