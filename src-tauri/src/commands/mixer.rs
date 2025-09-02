use crate::{
    ApplicationAudioState, AudioChannel, AudioConfigFactory, AudioMetrics, AudioState,
    MixerCommand, MixerConfig, VirtualMixer,
};
use tauri::State;
use tracing::error;

// Virtual mixer commands
#[tauri::command]
pub async fn create_mixer(
    audio_state: State<'_, AudioState>,
    config: MixerConfig,
) -> Result<(), String> {
    // **CRASH FIX**: Add comprehensive error handling for mixer creation
    println!(
        "🎛️ Creating mixer with {} channels...",
        config.channels.len()
    );

    // Create the mixer with enhanced error handling
    let mut mixer = match VirtualMixer::new(config).await {
        Ok(mixer) => {
            println!("✅ Mixer structure created successfully");
            mixer
        }
        Err(e) => {
            error!("Failed to create mixer: {}", e);
            return Err(format!("Failed to create mixer: {}", e));
        }
    };

    // **CRASH FIX**: Start the mixer with better error handling
    match mixer.start().await {
        Ok(()) => {
            println!("✅ Mixer started successfully (always-running mode)");
        }
        Err(e) => {
            error!("Failed to start mixer: {}", e);
            return Err(format!("Failed to start mixer: {}", e));
        }
    }

    // Store the initialized mixer
    *audio_state.mixer.lock().await = Some(mixer);
    println!("🎛️ Mixer created, started, and stored successfully");
    Ok(())
}

#[tauri::command]
pub async fn start_mixer(audio_state: State<'_, AudioState>) -> Result<(), String> {
    // DEPRECATED: Mixer is now always running after creation
    // This command is kept for compatibility but does nothing
    let mixer_guard = audio_state.mixer.lock().await;
    if mixer_guard.is_some() {
        println!("⚠️ start_mixer called but mixer is already always-running");
        Ok(())
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
pub async fn stop_mixer(audio_state: State<'_, AudioState>) -> Result<(), String> {
    // DEPRECATED: Mixer is now always running and cannot be stopped
    // This command is kept for compatibility but does nothing
    let mixer_guard = audio_state.mixer.lock().await;
    if mixer_guard.is_some() {
        println!("⚠️ stop_mixer called but mixer is always-running (operation ignored)");
        Ok(())
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
pub async fn add_mixer_channel(
    audio_state: State<'_, AudioState>,
    channel: AudioChannel,
) -> Result<(), String> {
    let mut mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mut mixer) = *mixer_guard {
        mixer
            .add_channel(channel)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn update_mixer_channel(
    audio_state: State<'_, AudioState>,
    app_audio_state: State<'_, ApplicationAudioState>,
    channel_id: u32,
    channel: AudioChannel,
) -> Result<(), String> {
    println!(
        "🎛️ UPDATE_MIXER_CHANNEL called for channel {} with device_id: {:?}",
        channel_id, channel.input_device_id
    );

    // Check if the device ID is an application source
    println!("🔧 DEBUG: Checking if device_id is Some...");
    if let Some(device_id) = &channel.input_device_id {
        println!("🔧 DEBUG: device_id is Some: '{}'", device_id);
        println!("🔧 DEBUG: Checking if device_id starts with 'app-'...");
        if device_id.starts_with("app-") {
            println!("🔧 DEBUG: device_id starts with 'app-', extracting PID...");
            // This is an application source - create a tap for it
            if let Ok(pid_str) = device_id.strip_prefix("app-").unwrap_or("").parse::<u32>() {
                println!("🎵 Creating audio tap for application PID: {}", pid_str);

                match app_audio_state
                    .manager
                    .create_mixer_input_for_app(pid_str)
                    .await
                {
                    Ok(channel_name) => {
                        println!(
                            "✅ Successfully created mixer input for PID {}: {}",
                            pid_str, channel_name
                        );
                        // Virtual stream is now registered and ready for mixer
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        println!(
                            "❌ Failed to create audio tap for PID {}: {}",
                            pid_str, error_msg
                        );

                        // Check if this is a permission error and provide helpful guidance
                        if error_msg.contains("Audio capture permissions not granted")
                            || error_msg.contains("permission")
                        {
                            return Err(format!(
                                "🎤 Audio capture permission required!\n\n\
                                To capture audio from applications, please:\n\
                                1. Open System Preferences → Security & Privacy → Privacy\n\
                                2. Select 'Microphone' from the left sidebar\n\
                                3. Find 'SendinBeats' in the list and check the box\n\
                                4. Restart the application\n\n\
                                This permission is required for Core Audio Taps to capture audio from other applications."
                            ));
                        }

                        // For other errors, return a generic error message
                        return Err(format!("Failed to create audio tap: {}", error_msg));
                    }
                }
            } else {
                println!("❌ Failed to parse PID from device_id: {}", device_id);
            }
        } else {
            println!(
                "🔧 DEBUG: device_id does NOT start with 'app-': '{}'",
                device_id
            );
        }
    } else {
        println!("🔧 DEBUG: device_id is None");
    }

    let mut mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mut mixer) = *mixer_guard {
        mixer
            .update_channel(channel_id, channel)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn get_mixer_metrics(audio_state: State<'_, AudioState>) -> Result<AudioMetrics, String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        Ok(mixer.get_metrics().await)
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
pub async fn get_channel_levels(
    audio_state: State<'_, AudioState>,
) -> Result<std::collections::HashMap<u32, (f32, f32, f32, f32)>, String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        Ok(mixer.get_channel_levels().await)
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
pub async fn get_master_levels(
    audio_state: State<'_, AudioState>,
) -> Result<(f32, f32, f32, f32), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        Ok(mixer.get_master_levels().await)
    } else {
        Err("No mixer created".to_string())
    }
}

#[tauri::command]
pub async fn send_mixer_command(
    audio_state: State<'_, AudioState>,
    command: MixerCommand,
) -> Result<(), String> {
    let mixer_guard = audio_state.mixer.lock().await;
    if let Some(ref mixer) = *mixer_guard {
        mixer
            .send_command(command)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        return Err("No mixer created".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn get_dj_mixer_config() -> MixerConfig {
    AudioConfigFactory::create_dj_config()
}

#[tauri::command]
pub fn get_streaming_mixer_config() -> MixerConfig {
    AudioConfigFactory::create_streaming_config()
}

#[tauri::command]
pub async fn check_audio_capture_permissions(
    app_audio_state: State<'_, ApplicationAudioState>,
) -> Result<bool, String> {
    let has_permission = app_audio_state.manager.has_permissions().await;
    Ok(has_permission)
}

// Helper function to trigger microphone permission request and add app to System Preferences
fn try_trigger_microphone_permission() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        println!("🎤 Attempting to trigger macOS permission dialog through AVAudioSession...");

        // Try using a more direct approach that forces the permission dialog
        use std::process::Command;

        // First, let's try to trigger a permission request using osascript to simulate
        // what a native app would do - this should force the system dialog
        let script = r#"
            tell application "System Events"
                try
                    -- This will trigger the microphone permission dialog
                    set microphoneAccess to (do shell script "echo 'test' | /usr/bin/say")
                    return "permission_triggered"
                on error
                    return "permission_denied"
                end try
            end tell
        "#;

        println!("🔐 Executing permission trigger script...");
        match Command::new("osascript").arg("-e").arg(script).output() {
            Ok(output) => {
                let result = String::from_utf8_lossy(&output.stdout);
                println!("📋 Script result: {}", result);

                // Fallback to cpal method
                try_cpal_microphone_access()
            }
            Err(e) => {
                println!("❌ AppleScript failed: {}, trying cpal method", e);
                try_cpal_microphone_access()
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Permission management is only available on macOS".to_string())
    }
}

// Fallback CPAL method for microphone access
#[cfg(target_os = "macos")]
fn try_cpal_microphone_access() -> Result<String, String> {
    use cpal::traits::{DeviceTrait, HostTrait};

    println!("🎤 Attempting CPAL microphone access...");

    match cpal::default_host().default_input_device() {
        Some(device) => {
            println!(
                "📱 Found default input device: {}",
                device.name().unwrap_or_default()
            );

            match device.default_input_config() {
                Ok(config) => {
                    println!("✅ Successfully accessed input device config");

                    let sample_rate = config.sample_rate();
                    let channels = config.channels();
                    println!(
                        "🔧 Building input stream (sample_rate: {}, channels: {})",
                        sample_rate.0, channels
                    );

                    match device.build_input_stream(
                        &config.into(),
                        move |_data: &[f32], _: &cpal::InputCallbackInfo| {
                            // Just access the microphone to trigger permission
                        },
                        |err| {
                            eprintln!("Stream error: {}", err);
                        },
                        None,
                    ) {
                        Ok(_stream) => {
                            println!("🎉 Input stream created successfully!");
                            println!("   If this is the first time, a permission dialog should have appeared");

                            // Keep the stream alive briefly
                            std::thread::sleep(std::time::Duration::from_millis(100));

                            Ok("microphone_accessed".to_string())
                        }
                        Err(e) => {
                            println!("❌ Failed to build input stream: {}", e);
                            Err(format!("Microphone access failed: {}", e))
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to get input config: {}", e);
                    Err(format!("Microphone configuration error: {}", e))
                }
            }
        }
        None => {
            println!("❌ No input device found");
            Err("No microphone device available".to_string())
        }
    }
}

// Try to force the permission dialog using multiple approaches
async fn try_force_permission_dialog() -> Result<bool, String> {
    use std::process::Command;

    println!("🔥 Attempting to force macOS permission dialog...");

    // Method 1: Try to record a very short audio snippet
    let result = Command::new("sh")
        .arg("-c")
        .arg(
            "timeout 1 sox -t coreaudio default /tmp/test_audio.wav trim 0 0.1 2>/dev/null || true",
        )
        .output();

    if let Ok(output) = result {
        println!("📱 Sox command result: {}", output.status);
        if output.status.success() {
            println!("✅ Sox succeeded - permission dialog should have appeared");
            return Ok(true);
        }
    }

    // Method 2: Try using ffmpeg to access microphone
    let result2 = Command::new("sh")
        .arg("-c") 
        .arg("timeout 1 ffmpeg -f avfoundation -i \":0\" -t 0.1 -y /tmp/test_audio2.wav 2>/dev/null || true")
        .output();

    if let Ok(output2) = result2 {
        println!("🎬 FFmpeg command result: {}", output2.status);
        if output2.status.success() {
            println!("✅ FFmpeg succeeded - permission dialog should have appeared");
            return Ok(true);
        }
    }

    println!("❌ Neither sox nor ffmpeg triggered permission dialog");
    Ok(false)
}

#[tauri::command]
pub async fn request_audio_capture_permissions(
    app_audio_state: State<'_, ApplicationAudioState>,
) -> Result<String, String> {
    println!("🔐 request_audio_capture_permissions: Starting permission request...");

    let has_permission = app_audio_state.manager.has_permissions().await;

    if has_permission {
        println!("✅ Permissions already granted");
        Ok("Audio capture permission already granted".to_string())
    } else {
        println!("⚠️ Permissions not granted, attempting to trigger permission request...");

        // Try one more aggressive approach to trigger the dialog
        println!("🔐 Trying aggressive permission trigger...");

        match try_force_permission_dialog().await {
            Ok(dialog_shown) => {
                if dialog_shown {
                    Ok("✅ Permission dialog should have appeared! Check System Settings → Privacy & Security → Microphone".to_string())
                } else {
                    Ok(format!(
            "🔧 DEVELOPMENT BUILD PERMISSION SETUP\n\n\
            Since this is a development build, manually add the app to System Settings:\n\n\
            FOR macOS 13+ (Ventura/Sonoma):\n\
            1. Open System Settings (not System Preferences)\n\
            2. Go to Privacy & Security → Microphone\n\
            3. Look for a '+' button or toggle to add applications\n\
            4. Navigate to: /Users/aaron.wilson/code/sending-beats/src-tauri/target/debug/\n\
            5. Select 'SendinBeats' binary and enable it\n\n\
            FOR older macOS:\n\
            1. Open System Preferences → Security & Privacy → Privacy\n\
            2. Click 'Microphone', unlock with password if needed\n\
            3. Click '+' to add the binary from the path above\n\n\
            ALTERNATIVE: Try running this in Terminal:\n\
            sudo tccutil reset Microphone\n\
            Then click this button again - it might trigger the dialog!"
                    ))
                }
            }
            Err(_) => {
                Ok("❌ Automatic permission trigger failed. Please manually add the app to System Settings → Privacy & Security → Microphone".to_string())
            }
        }
    }
}

#[tauri::command]
pub async fn open_system_preferences_privacy() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        // Try to open System Preferences directly to Privacy settings
        match Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
            .output()
        {
            Ok(_) => Ok("System Preferences opened".to_string()),
            Err(e) => {
                eprintln!("Failed to open System Preferences: {}", e);
                // Fallback - open general System Preferences
                match Command::new("open")
                    .arg("/System/Library/PreferencePanes/Security.prefPane")
                    .output()
                {
                    Ok(_) => Ok("System Preferences opened (general)".to_string()),
                    Err(e2) => Err(format!("Failed to open System Preferences: {}", e2)),
                }
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("System Preferences only available on macOS".to_string())
    }
}
