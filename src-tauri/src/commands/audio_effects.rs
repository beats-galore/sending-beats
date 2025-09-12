use crate::AudioState;
use tauri::State;

// Audio effects management commands
#[tauri::command]
pub async fn update_channel_eq(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    eq_low_gain: Option<f32>,
    eq_mid_gain: Option<f32>,
    eq_high_gain: Option<f32>,
) -> Result<(), String> {
    Ok(())
    // let mut mixer_guard = audio_state.mixer.lock().await;
    // if let Some(ref mut mixer) = *mixer_guard {
    //     // Clone the current channel first
    //     let mut updated_channel = if let Some(channel) = mixer.get_channel(channel_id) {
    //         channel.clone()
    //     } else {
    //         return Err(format!("Channel {} not found", channel_id));
    //     };

    //     // Update EQ settings
    //     if let Some(gain) = eq_low_gain {
    //         updated_channel.eq_low_gain = gain.clamp(-12.0, 12.0);
    //     }
    //     if let Some(gain) = eq_mid_gain {
    //         updated_channel.eq_mid_gain = gain.clamp(-12.0, 12.0);
    //     }
    //     if let Some(gain) = eq_high_gain {
    //         updated_channel.eq_high_gain = gain.clamp(-12.0, 12.0);
    //     }

    //     // **CRITICAL FIX**: Auto-enable effects when EQ is modified (this was missing!)
    //     updated_channel.effects_enabled = true;

    //     // Update the channel in the mixer to trigger real-time changes
    //     mixer
    //         .update_channel(channel_id, updated_channel.clone())
    //         .await
    //         .map_err(|e| e.to_string())?;
    //     println!(
    //         "🎛️ Updated EQ for channel {}: low={:.1}, mid={:.1}, high={:.1}",
    //         channel_id,
    //         updated_channel.eq_low_gain,
    //         updated_channel.eq_mid_gain,
    //         updated_channel.eq_high_gain
    //     );

    //     Ok(())
    // } else {
    //     Err("No mixer created".to_string())
    // }
}

#[tauri::command]
pub async fn update_channel_compressor(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    threshold: Option<f32>,
    ratio: Option<f32>,
    attack_ms: Option<f32>,
    release_ms: Option<f32>,
    enabled: Option<bool>,
) -> Result<(), String> {
    Ok(())
    // let mut mixer_guard = audio_state.mixer.lock().await;
    // if let Some(ref mut mixer) = *mixer_guard {
    //     // Clone the current channel first
    //     let mut updated_channel = if let Some(channel) = mixer.get_channel(channel_id) {
    //         channel.clone()
    //     } else {
    //         return Err(format!("Channel {} not found", channel_id));
    //     };

    //     // Update compressor settings
    //     if let Some(thresh) = threshold {
    //         updated_channel.comp_threshold = thresh.clamp(-40.0, 0.0);
    //     }
    //     if let Some(r) = ratio {
    //         updated_channel.comp_ratio = r.clamp(1.0, 10.0);
    //     }
    //     if let Some(attack) = attack_ms {
    //         updated_channel.comp_attack = attack.clamp(0.1, 100.0);
    //     }
    //     if let Some(release) = release_ms {
    //         updated_channel.comp_release = release.clamp(10.0, 1000.0);
    //     }
    //     if let Some(en) = enabled {
    //         updated_channel.comp_enabled = en;
    //     }

    //     // **CRITICAL FIX**: Auto-enable effects when compressor is modified
    //     updated_channel.effects_enabled = true;

    //     // Update the channel in the mixer to trigger real-time changes
    //     mixer
    //         .update_channel(channel_id, updated_channel.clone())
    //         .await
    //         .map_err(|e| e.to_string())?;
    //     println!("🎛️ Updated compressor for channel {}: threshold={:.1}dB, ratio={:.1}:1, attack={:.1}ms, release={:.0}ms, enabled={}",
    //         channel_id, updated_channel.comp_threshold, updated_channel.comp_ratio, updated_channel.comp_attack, updated_channel.comp_release, updated_channel.comp_enabled);

    //     Ok(())
    // } else {
    //     Err("No mixer created".to_string())
    // }
}

#[tauri::command]
pub async fn update_channel_limiter(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    threshold_db: Option<f32>,
    enabled: Option<bool>,
) -> Result<(), String> {
    Ok(())
    // let mut mixer_guard = audio_state.mixer.lock().await;
    // if let Some(ref mut mixer) = *mixer_guard {
    //     // Clone the current channel first
    //     let mut updated_channel = if let Some(channel) = mixer.get_channel(channel_id) {
    //         channel.clone()
    //     } else {
    //         return Err(format!("Channel {} not found", channel_id));
    //     };

    //     // Update limiter settings
    //     if let Some(thresh) = threshold_db {
    //         updated_channel.limiter_threshold = thresh.clamp(-12.0, 0.0);
    //     }
    //     if let Some(en) = enabled {
    //         updated_channel.limiter_enabled = en;
    //     }

    //     // **CRITICAL FIX**: Auto-enable effects when limiter is modified
    //     updated_channel.effects_enabled = true;

    //     // Update the channel in the mixer to trigger real-time changes
    //     mixer
    //         .update_channel(channel_id, updated_channel.clone())
    //         .await
    //         .map_err(|e| e.to_string())?;
    //     println!(
    //         "🎛️ Updated limiter for channel {}: threshold={:.1}dB, enabled={}",
    //         channel_id, updated_channel.limiter_threshold, updated_channel.limiter_enabled
    //     );

    //     Ok(())
    // } else {
    //     Err("No mixer created".to_string())
    // }
}

// Effects management commands - add/remove individual effects
#[tauri::command]
pub async fn add_channel_effect(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    effect_type: String, // "eq", "compressor", "limiter"
) -> Result<(), String> {
    Ok(())
    // let mut mixer_guard = audio_state.mixer.lock().await;
    // if let Some(ref mut mixer) = *mixer_guard {
    //     // Clone the current channel first
    //     let mut updated_channel = if let Some(channel) = mixer.get_channel(channel_id) {
    //         channel.clone()
    //     } else {
    //         return Err(format!("Channel {} not found", channel_id));
    //     };

    //     match effect_type.as_str() {
    //         "eq" => {
    //             // Reset EQ to flat response (effectively "adding" it)
    //             updated_channel.eq_low_gain = 0.0;
    //             updated_channel.eq_mid_gain = 0.0;
    //             updated_channel.eq_high_gain = 0.0;
    //             println!("➕ Added EQ to channel {}", channel_id);
    //         }
    //         "compressor" => {
    //             // Enable compressor with default settings
    //             updated_channel.comp_enabled = true;
    //             updated_channel.comp_threshold = -12.0;
    //             updated_channel.comp_ratio = 4.0;
    //             updated_channel.comp_attack = 10.0;
    //             updated_channel.comp_release = 100.0;
    //             println!("➕ Added compressor to channel {}", channel_id);
    //         }
    //         "limiter" => {
    //             // Enable limiter with default settings
    //             updated_channel.limiter_enabled = true;
    //             updated_channel.limiter_threshold = -3.0;
    //             println!("➕ Added limiter to channel {}", channel_id);
    //         }
    //         _ => return Err(format!("Unknown effect type: {}", effect_type)),
    //     }

    //     // **CRITICAL FIX**: Auto-enable effects when any effect is added
    //     updated_channel.effects_enabled = true;

    //     // Update the channel in the mixer to trigger real-time changes
    //     mixer
    //         .update_channel(channel_id, updated_channel.clone())
    //         .await
    //         .map_err(|e| e.to_string())?;
    //     Ok(())
    // } else {
    //     Err("No mixer created".to_string())
    // }
}

#[tauri::command]
pub async fn remove_channel_effect(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
    effect_type: String, // "eq", "compressor", "limiter"
) -> Result<(), String> {
    Ok(())
    // let mut mixer_guard = audio_state.mixer.lock().await;
    // if let Some(ref mut mixer) = *mixer_guard {
    //     // Clone the current channel first
    //     let mut updated_channel = if let Some(channel) = mixer.get_channel(channel_id) {
    //         channel.clone()
    //     } else {
    //         return Err(format!("Channel {} not found", channel_id));
    //     };

    //     match effect_type.as_str() {
    //         "eq" => {
    //             // Reset EQ to flat response (effectively "removing" it)
    //             updated_channel.eq_low_gain = 0.0;
    //             updated_channel.eq_mid_gain = 0.0;
    //             updated_channel.eq_high_gain = 0.0;
    //             println!("➖ Removed EQ from channel {}", channel_id);
    //         }
    //         "compressor" => {
    //             // Disable compressor
    //             updated_channel.comp_enabled = false;
    //             println!("➖ Removed compressor from channel {}", channel_id);
    //         }
    //         "limiter" => {
    //             // Disable limiter
    //             updated_channel.limiter_enabled = false;
    //             println!("➖ Removed limiter from channel {}", channel_id);
    //         }
    //         _ => return Err(format!("Unknown effect type: {}", effect_type)),
    //     }

    //     // Update the channel in the mixer to trigger real-time changes
    //     mixer
    //         .update_channel(channel_id, updated_channel.clone())
    //         .await
    //         .map_err(|e| e.to_string())?;
    //     Ok(())
    // } else {
    //     Err("No mixer created".to_string())
    // }
}

#[tauri::command]
pub async fn get_channel_effects(
    audio_state: State<'_, AudioState>,
    channel_id: u32,
) -> Result<(), String> {
    Ok(())
    // let mixer_guard = audio_state.mixer.lock().await;
    // if let Some(ref mixer) = *mixer_guard {
    //     if let Some(channel) = mixer.get_channel(channel_id) {
    //         let mut effects = Vec::new();

    //         // Check which effects are active
    //         if channel.eq_low_gain != 0.0
    //             || channel.eq_mid_gain != 0.0
    //             || channel.eq_high_gain != 0.0
    //         {
    //             effects.push("eq".to_string());
    //         }
    //         if channel.comp_enabled {
    //             effects.push("compressor".to_string());
    //         }
    //         if channel.limiter_enabled {
    //             effects.push("limiter".to_string());
    //         }

    //         Ok(effects)
    //     } else {
    //         Err(format!("Channel {} not found", channel_id))
    //     }
    // } else {
    //     Err("No mixer created".to_string())
    // }
}
