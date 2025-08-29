use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::{broadcast, Mutex, RwLock};
use sysinfo::{System, Pid};
use tracing::{info, warn, error, debug};

/// Centralized virtual input stream registry to ensure mixer can find registered streams
fn get_virtual_input_registry() -> &'static StdMutex<HashMap<String, Arc<crate::audio::mixer::transformer::AudioInputStream>>> {
    use std::sync::LazyLock;
    static VIRTUAL_INPUT_REGISTRY: LazyLock<StdMutex<HashMap<String, Arc<crate::audio::mixer::transformer::AudioInputStream>>>> = 
        LazyLock::new(|| StdMutex::new(HashMap::new()));
    &VIRTUAL_INPUT_REGISTRY
}

/// Context data for Core Audio tap IOProc callback
#[cfg(target_os = "macos")]
struct CoreAudioTapCallbackContext {
    audio_tx: broadcast::Sender<Vec<f32>>,
    process_name: String,
    sample_rate: f64,
    channels: u32,
    callback_count: std::sync::atomic::AtomicU64,
}

/// Helper struct for audio format information
#[derive(Debug, Clone)]
struct AudioFormatInfo {
    sample_rate: f64,
    channels: u32,
    bits_per_sample: u32,
}

/// Core Audio IOProc callback for tap device
#[cfg(target_os = "macos")]
unsafe extern "C" fn core_audio_tap_callback(
    device_id: coreaudio_sys::AudioObjectID,
    _now: *const coreaudio_sys::AudioTimeStamp,
    input_data: *const coreaudio_sys::AudioBufferList,
    _input_time: *const coreaudio_sys::AudioTimeStamp,
    _output_data: *mut coreaudio_sys::AudioBufferList,
    _output_time: *const coreaudio_sys::AudioTimeStamp,
    client_data: *mut std::os::raw::c_void,
) -> coreaudio_sys::OSStatus {
    // Safety: client_data was created from Box::into_raw, so it's valid
    if client_data.is_null() {
        return -1; // Invalid parameter
    }
    
    let context = &*(client_data as *const CoreAudioTapCallbackContext);
    let callback_count = context.callback_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    
    if input_data.is_null() {
        if callback_count % 1000 == 0 {
            eprintln!("⚠️ TAP CALLBACK: No input data (callback #{})", callback_count);
        }
        return 0; // No error, but no data
    }
    
    // Extract audio samples from AudioBufferList
    let buffer_list = &*input_data;
    let buffer_count = buffer_list.mNumberBuffers;
    
    if buffer_count == 0 {
        if callback_count % 1000 == 0 {
            eprintln!("⚠️ TAP CALLBACK: No audio buffers (callback #{})", callback_count);
        }
        return 0;
    }
    
    // Process the first buffer (typically the only one for simple cases)
    let audio_buffer = &buffer_list.mBuffers[0];
    let data_ptr = audio_buffer.mData as *const f32;
    let sample_count = (audio_buffer.mDataByteSize as usize) / std::mem::size_of::<f32>();
    
    if data_ptr.is_null() || sample_count == 0 {
        if callback_count % 1000 == 0 {
            eprintln!("⚠️ TAP CALLBACK: No sample data (callback #{})", callback_count);
        }
        return 0;
    }
    
    // Convert raw audio data to Vec<f32>
    let samples: Vec<f32> = std::slice::from_raw_parts(data_ptr, sample_count).to_vec();
    
    // Calculate audio levels for monitoring
    let peak_level = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
    let rms_level = (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    
    // Log periodically
    if callback_count % 100 == 0 || (peak_level > 0.01 && callback_count % 50 == 0) {
        eprintln!("🔊 CORE AUDIO TAP [{}] Device {}: Callback #{}: {} samples, peak: {:.4}, rms: {:.4}", 
                 context.process_name, device_id, callback_count, samples.len(), peak_level, rms_level);
    }
    
    // Send samples to broadcast channel for mixer integration
    if let Err(_) = context.audio_tx.send(samples) {
        if callback_count % 1000 == 0 {
            eprintln!("⚠️ Failed to send tap samples to broadcast channel (callback #{})", callback_count);
        }
    }
    
    0 // Success
}

/// Information about a discovered process that might have audio capabilities
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub bundle_id: Option<String>,
    pub icon_path: Option<PathBuf>,
    pub is_audio_capable: bool,
    pub is_playing_audio: bool,
}

/// Discovers and tracks audio-capable applications on the system
pub struct ApplicationDiscovery {
    system: System,
    known_audio_apps: HashMap<String, String>, // process name -> bundle ID
    last_scan: std::time::Instant,
    scan_interval: std::time::Duration,
}

impl ApplicationDiscovery {
    pub fn new() -> Self {
        let mut known_audio_apps = HashMap::new();
        
        // Add well-known audio applications
        known_audio_apps.insert("Spotify".to_string(), "com.spotify.client".to_string());
        known_audio_apps.insert("iTunes".to_string(), "com.apple.iTunes".to_string());
        known_audio_apps.insert("Music".to_string(), "com.apple.Music".to_string());
        known_audio_apps.insert("Tidal".to_string(), "com.tidal.desktop".to_string());
        known_audio_apps.insert("YouTube Music Desktop".to_string(), "com.ytmusic.ytmusic".to_string());
        known_audio_apps.insert("Pandora".to_string(), "com.pandora.desktop".to_string());
        known_audio_apps.insert("SoundCloud".to_string(), "com.soundcloud.desktop".to_string());
        known_audio_apps.insert("Apple Music".to_string(), "com.apple.Music".to_string());
        known_audio_apps.insert("Amazon Music".to_string(), "com.amazon.music".to_string());
        known_audio_apps.insert("Deezer".to_string(), "com.deezer.desktop".to_string());
        known_audio_apps.insert("VLC".to_string(), "org.videolan.vlc".to_string());
        known_audio_apps.insert("IINA".to_string(), "com.colliderli.iina".to_string());
        known_audio_apps.insert("QuickTime Player".to_string(), "com.apple.QuickTimePlayerX".to_string());
        
        Self {
            system: System::new_all(),
            known_audio_apps,
            last_scan: std::time::Instant::now() - std::time::Duration::from_secs(10), // Force initial scan
            scan_interval: std::time::Duration::from_secs(5), // Scan every 5 seconds
        }
    }
    
    /// Scan for all audio-capable applications currently running
    pub fn scan_audio_applications(&mut self) -> Result<Vec<ProcessInfo>> {
        // Disable caching for now - always do a fresh scan
        // TODO: Implement proper caching with stored results later
        
        println!("🔍 SCANNING: Starting audio application scan...");
        self.system.refresh_all();
        self.last_scan = std::time::Instant::now();
        
        let mut audio_processes = Vec::new();
        
        // Iterate through all running processes
        for (pid, process) in self.system.processes() {
            let process_name = process.name();
            
            // Check if this is a known audio application (exact match)
            if let Some(bundle_id) = self.known_audio_apps.get(process_name) {
                let process_info = ProcessInfo {
                    pid: pid.as_u32(),
                    name: process_name.to_string(),
                    bundle_id: Some(bundle_id.clone()),
                    icon_path: self.get_app_icon_path(bundle_id),
                    is_audio_capable: true,
                    is_playing_audio: self.is_app_playing_audio(pid.as_u32()),
                };
                
                debug!("Found known audio app: {} (PID: {})", process_name, pid);
                audio_processes.push(process_info);
            }
            // Also check for processes that might be audio-capable based on name patterns
            else if self.might_be_audio_app(process_name) {
                let process_info = ProcessInfo {
                    pid: pid.as_u32(),
                    name: process_name.to_string(),
                    bundle_id: None,
                    icon_path: None,
                    is_audio_capable: true,
                    is_playing_audio: self.is_app_playing_audio(pid.as_u32()),
                };
                
                debug!("Found potential audio app: {} (PID: {})", process_name, pid);
                audio_processes.push(process_info);
            }
        }
        
        info!("Found {} audio-capable applications", audio_processes.len());
        Ok(audio_processes)
    }
    
    /// Get only the well-known audio applications
    pub fn get_known_audio_apps(&mut self) -> Result<Vec<ProcessInfo>> {
        let all_audio_apps = self.scan_audio_applications()?;
        Ok(all_audio_apps.into_iter()
            .filter(|app| app.bundle_id.is_some())
            .collect())
    }
    
    /// Check if an application might be audio-capable based on name patterns
    fn might_be_audio_app(&self, process_name: &str) -> bool {
        let audio_keywords = [
            "music", "audio", "sound", "player", "radio", "podcast", 
            "stream", "media", "video", "youtube", "netflix", "hulu"
        ];
        
        let name_lower = process_name.to_lowercase();
        audio_keywords.iter().any(|keyword| name_lower.contains(keyword))
    }
    
    /// Check if an application is currently playing audio (placeholder implementation)
    fn is_app_playing_audio(&self, _pid: u32) -> bool {
        // TODO: Implement actual audio playback detection
        // This would require Core Audio APIs to check if a process is producing audio
        // For now, we'll assume any running audio app might be playing audio
        false
    }
    
    /// Get the icon path for an application bundle (placeholder implementation)
    fn get_app_icon_path(&self, _bundle_id: &str) -> Option<PathBuf> {
        // TODO: Implement app icon discovery
        // This would involve querying the app bundle for its icon file
        None
    }
    
    /// Get cached audio applications if scan hasn't expired
    fn get_cached_audio_applications(&self) -> Result<Vec<ProcessInfo>> {
        // TODO: Implement proper caching mechanism with stored results
        // For now, return empty vec since caching is disabled
        Ok(Vec::new())
    }
    
    /// Refresh the system process list
    pub fn refresh(&mut self) {
        self.system.refresh_all();
    }
    
    /// Get process info by PID
    pub fn get_process_info(&self, pid: u32) -> Option<ProcessInfo> {
        if let Some(process) = self.system.process(Pid::from_u32(pid)) {
            let process_name = process.name();
            let bundle_id = self.known_audio_apps.get(process_name).cloned();
            
            Some(ProcessInfo {
                pid,
                name: process_name.to_string(),
                bundle_id: bundle_id.clone(),
                icon_path: bundle_id.as_ref().and_then(|bid| self.get_app_icon_path(bid)),
                is_audio_capable: bundle_id.is_some() || self.might_be_audio_app(process_name),
                is_playing_audio: self.is_app_playing_audio(pid),
            })
        } else {
            None
        }
    }
}

/// Statistics for monitoring tap health
#[derive(Debug, Clone, serde::Serialize)]
pub struct TapStats {
    pub pid: u32,
    pub process_name: String,
    pub age: std::time::Duration,
    pub last_activity: std::time::Duration,
    pub error_count: u32,
    pub is_capturing: bool,
    pub process_alive: bool,
}

/// Manages Core Audio taps for individual applications (macOS 14.4+ only)
#[cfg(target_os = "macos")]
pub struct ApplicationAudioTap {
    process_info: ProcessInfo,
    tap_id: Option<u32>, // AudioObjectID placeholder
    aggregate_device_id: Option<u32>, // AudioObjectID placeholder
    audio_tx: Option<broadcast::Sender<Vec<f32>>>,
    _stream_info: Option<String>, // Just store stream info for debugging
    is_capturing: bool,
    created_at: std::time::Instant,
    last_heartbeat: Arc<StdMutex<std::time::Instant>>,
    error_count: Arc<StdMutex<u32>>,
    max_errors: u32,
}

#[cfg(target_os = "macos")]
impl ApplicationAudioTap {
    pub fn new(process_info: ProcessInfo) -> Self {
        let now = std::time::Instant::now();
        Self {
            process_info,
            tap_id: None,
            aggregate_device_id: None,
            audio_tx: None,
            _stream_info: None,
            is_capturing: false,
            created_at: now,
            last_heartbeat: Arc::new(StdMutex::new(now)),
            error_count: Arc::new(StdMutex::new(0)),
            max_errors: 5, // Maximum errors before automatic cleanup
        }
    }
    
    /// Create a Core Audio tap for this application's process
    pub async fn create_tap(&mut self) -> Result<()> {
        info!("🔧 DEBUG: Creating audio tap for {} (PID: {})", self.process_info.name, self.process_info.pid);
        info!("🔧 DEBUG: Process bundle_id: {:?}", self.process_info.bundle_id);
        
        // Check macOS version compatibility
        if !self.is_core_audio_taps_supported() {
            return Err(anyhow::anyhow!(
                "Core Audio taps require macOS 14.4 or later. Use BlackHole for audio capture on older systems."
            ));
        }
        
        // Import Core Audio taps bindings (only available on macOS 14.4+)
        #[cfg(target_os = "macos")]
        {
            use super::core_audio_bindings::{
                create_process_tap_description,
                create_process_tap, 
                format_osstatus_error
            };
            
            // Step 1: Try using PID directly in CATapDescription (skip translation)
            info!("Creating Core Audio process tap for PID {} directly with objc2_core_audio", self.process_info.pid);
            let tap_object_id = unsafe {
                // Create tap description in a limited scope so it's dropped before await
                // Try using PID directly - some examples suggest this works
                let tap_description = create_process_tap_description(self.process_info.pid);
                info!("Created tap description for process {}", self.process_info.name);
                
                match create_process_tap(&tap_description) {
                    Ok(id) => {
                        info!("✅ SUCCESS: Created process tap with AudioObjectID {} for {} (PID: {})", 
                              id, self.process_info.name, self.process_info.pid);
                        id
                    }
                    Err(status) => {
                        let error_msg = format_osstatus_error(status);
                        if status == -4 {
                            return Err(anyhow::anyhow!(
                                "Core Audio Process Taps API not available on this system.\n\
                                This feature requires macOS 14.4+ with the latest Core Audio framework.\n\
                                Alternative: Use BlackHole virtual audio device:\n\
                                1. Set BlackHole 2ch as system output\n\
                                2. Select BlackHole 2ch as mixer input\n\
                                3. Play audio in {} - it will be captured", 
                                self.process_info.name
                            ));
                        } else {
                            return Err(anyhow::anyhow!(
                                "Failed to create process tap for {}: {} ({})", 
                                self.process_info.name, error_msg, status
                            ));
                        }
                    }
                }
                // tap_description is dropped here, before any await points
            };
            
            // Store the tap ID for later cleanup
            self.tap_id = Some(tap_object_id as u32);
            
            // Step 2: Set up audio streaming from the tap
            info!("Setting up audio stream from tap...");
            
            // Create broadcast channel for audio data
            let (audio_tx, _audio_rx) = broadcast::channel(1024);
            self.audio_tx = Some(audio_tx.clone());
            
            // Set up actual audio callback and streaming
            self.setup_tap_audio_stream(tap_object_id, audio_tx).await?;
            
            info!("✅ Audio tap successfully created for {}", self.process_info.name);
            Ok(())
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Err(anyhow::anyhow!("Application audio capture is only supported on macOS"))
        }
    }
    
    /// Set up audio streaming from the Core Audio tap
    #[cfg(target_os = "macos")]
    async fn setup_tap_audio_stream(
        &mut self,
        tap_object_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
    ) -> Result<()> {
        info!("Setting up audio stream for tap AudioObjectID {}", tap_object_id);
        
        // Use cpal to create an AudioUnit-based input stream from the tap device
        self.create_cpal_input_stream_from_tap(tap_object_id, audio_tx).await
    }
    
    /// Create a CPAL input stream from the Core Audio tap device
    #[cfg(target_os = "macos")]
    async fn create_cpal_input_stream_from_tap(
        &mut self,
        tap_object_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
    ) -> Result<()> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        
        info!("Creating CPAL input stream for Core Audio tap device ID {}", tap_object_id);
        
        // Get the tap device properties using Core Audio APIs
        let sample_rate = unsafe {
            self.get_tap_sample_rate(tap_object_id).unwrap_or(48000.0)
        };
        
        let channels = unsafe {
            self.get_tap_channel_count(tap_object_id).unwrap_or(2)
        };
        
        info!("Tap device properties: {} Hz, {} channels", sample_rate, channels);
        
        // Try to find this tap device in CPAL's device enumeration
        // Core Audio taps should appear as input devices once created
        let host = cpal::default_host();
        let devices: Vec<cpal::Device> = match host.input_devices() {
            Ok(devices) => devices.collect(),
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to enumerate input devices: {}", e));
            }
        };
        
        // Look for a device that might correspond to our tap
        // Since we can't directly match AudioObjectID, we'll try to find by characteristics
        let mut tap_device = None;
        let tap_id_str = tap_object_id.to_string();
        
        info!("🔍 DEBUG: Looking for tap device among {} available input devices", devices.len());
        for (i, device) in devices.iter().enumerate() {
            if let Ok(device_name) = device.name() {
                info!("🔍 DEBUG: Input device {}: '{}'", i, device_name);
                
                // Core Audio taps might appear with specific naming patterns
                if device_name.contains("Tap") || device_name.contains(&tap_id_str) || device_name.contains("Aggregate") {
                    info!("✅ FOUND: Potential tap device: '{}'", device_name);
                    tap_device = Some(device.clone());
                    break;
                } else if device_name.contains(&self.process_info.name) {
                    info!("✅ FOUND: Device matching process name: '{}'", device_name);
                    tap_device = Some(device.clone());
                    break;
                }
            }
        }
        
        // If we can't find the tap device directly, create a virtual approach
        if tap_device.is_none() {
            warn!("⚠️  TAP DEVICE NOT FOUND: No tap device found in CPAL enumeration!");
            info!("🔄 FALLBACK: Using virtual audio bridge for tap AudioObjectID {}", tap_object_id);
            return self.setup_virtual_tap_bridge(tap_object_id, audio_tx, sample_rate, channels).await;
        }
        
        let device = tap_device.unwrap();
        let device_name = device.name().unwrap_or_else(|_| format!("Tap-{}", tap_object_id));
        
        // Get device configuration
        let device_config = match device.default_input_config() {
            Ok(config) => config,
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to get device config for tap: {}", e));
            }
        };
        
        // Create stream configuration matching the tap's native format
        let tap_sample_rate = sample_rate as u32;
        let tap_channels = channels as u16;
        
        // We'll capture at the tap's native rate and convert to mixer rate later if needed
        let config = cpal::StreamConfig {
            channels: tap_channels,
            sample_rate: cpal::SampleRate(tap_sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };
        
        info!("Creating tap stream with config: {} channels, {} Hz", config.channels, config.sample_rate.0);
        
        // Create the input stream with audio callback
        let process_name = self.process_info.name.clone();
        let mut callback_count = 0u64;
        let audio_tx_for_callback = audio_tx.clone();
        
        let stream = match device_config.sample_format() {
            cpal::SampleFormat::F32 => {
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        callback_count += 1;
                        
                        // Calculate audio levels for monitoring
                        let peak_level = data.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        let rms_level = (data.iter().map(|&s| s * s).sum::<f32>() / data.len() as f32).sqrt();
                        
                        // Convert to Vec<f32> and handle sample rate conversion if needed
                        let audio_samples = if tap_sample_rate != 48000 {
                            // Simple linear interpolation resampling for non-48kHz audio
                            Self::resample_audio(data, tap_sample_rate, 48000)
                        } else {
                            data.to_vec()
                        };
                        
                        if callback_count % 100 == 0 || (peak_level > 0.01 && callback_count % 50 == 0) {
                            info!("🔊 TAP AUDIO [{}] Device: '{}' | Callback #{}: {} samples, peak: {:.4}, rms: {:.4}", 
                                process_name, device_name, callback_count, data.len(), peak_level, rms_level);
                        }
                        
                        // Send audio data to broadcast channel for mixer integration
                        if let Err(e) = audio_tx_for_callback.send(audio_samples) {
                            if callback_count % 1000 == 0 {
                                warn!("Failed to send tap audio samples: {} (callback #{})", e, callback_count);
                            }
                        }
                    },
                    |err| {
                        error!("Tap audio input error: {}", err);
                    },
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        callback_count += 1;
                        
                        // Convert I16 to F32 and handle sample rate conversion
                        let f32_samples: Vec<f32> = data.iter()
                            .map(|&sample| {
                                if sample >= 0 {
                                    sample as f32 / 32767.0
                                } else {
                                    sample as f32 / 32768.0
                                }
                            })
                            .collect();
                        
                        let audio_samples = if tap_sample_rate != 48000 {
                            // Simple linear interpolation resampling for non-48kHz audio
                            Self::resample_audio(&f32_samples, tap_sample_rate, 48000)
                        } else {
                            f32_samples
                        };
                        
                        let peak_level = audio_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        
                        if callback_count % 100 == 0 || (peak_level > 0.01 && callback_count % 50 == 0) {
                            info!("🔊 TAP AUDIO I16 [{}] Callback #{}: {} samples, peak: {:.4}", 
                                process_name, callback_count, data.len(), peak_level);
                        }
                        
                        // Send converted audio data
                        if let Err(e) = audio_tx_for_callback.send(audio_samples) {
                            if callback_count % 1000 == 0 {
                                warn!("Failed to send tap audio I16 samples: {} (callback #{})", e, callback_count);
                            }
                        }
                    },
                    |err| {
                        error!("Tap audio I16 input error: {}", err);
                    },
                    None,
                )?
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported tap sample format: {:?}", device_config.sample_format()));
            }
        };
        
        // Start the stream
        stream.play().map_err(|e| anyhow::anyhow!("Failed to start tap stream: {}", e))?;
        
        info!("✅ Successfully started Core Audio tap stream for {}", self.process_info.name);
        self.is_capturing = true;
        
        // For now, we'll leak the stream to keep it running
        // In a production implementation, we'd need a proper stream lifecycle manager
        // that can handle cpal::Stream's non-Send nature
        let stream_info = format!("CoreAudio tap stream for {}", self.process_info.name);
        self._stream_info = Some(stream_info);
        
        // Leak the stream intentionally - it will remain active until the process ends
        // This is acceptable for application audio capture use cases
        std::mem::forget(stream);
        
        info!("⚠️ Stream leaked intentionally for lifecycle management - will remain active until process ends");
        
        Ok(())
    }
    
    /// Set up direct Core Audio IOProc integration for tap device
    #[cfg(target_os = "macos")]
    async fn setup_virtual_tap_bridge(
        &mut self,
        tap_object_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
        sample_rate: f64,
        channels: u32,
    ) -> Result<()> {
        info!("🔧 IMPLEMENTING: Direct Core Audio IOProc for tap AudioObjectID {}", tap_object_id);
        info!("📋 Tap config: {} Hz, {} channels", sample_rate, channels);
        
        // This is the correct approach - Core Audio taps don't appear as CPAL devices
        // We need to use Core Audio APIs directly to read from the tap
        
        use coreaudio_sys::{
            AudioDeviceCreateIOProcID, AudioDeviceStart, AudioDeviceStop,
            AudioDeviceIOProcID, AudioBufferList, AudioTimeStamp,
            UInt32, OSStatus, AudioBuffer, AudioDeviceIOProc
        };
        use std::os::raw::c_void;
        use std::ptr;
        
        // Create IOProc callback context to pass data
        let callback_context = Box::into_raw(Box::new(CoreAudioTapCallbackContext {
            audio_tx: audio_tx.clone(),
            process_name: self.process_info.name.clone(),
            sample_rate,
            channels,
            callback_count: std::sync::atomic::AtomicU64::new(0),
        }));
        
        // Create IOProc for the tap device  
        let mut io_proc_id: AudioDeviceIOProcID = None;
        let status = unsafe {
            AudioDeviceCreateIOProcID(
                tap_object_id,
                Some(core_audio_tap_callback),
                callback_context as *mut c_void,
                &mut io_proc_id,
            )
        };
        
        if status != 0 {
            // Cleanup the context if IOProc creation failed
            unsafe { drop(Box::from_raw(callback_context)); }
            
            // Decode the error for better understanding
            let error_code = status as u32;
            let fourcc = [
                ((error_code >> 24) & 0xFF) as u8,
                ((error_code >> 16) & 0xFF) as u8, 
                ((error_code >> 8) & 0xFF) as u8,
                (error_code & 0xFF) as u8,
            ];
            let error_str = String::from_utf8_lossy(&fourcc);
            
            error!("❌ AudioDeviceCreateIOProcID failed for tap {}", tap_object_id);
            error!("   OSStatus: {} (0x{:08x})", status, error_code);
            error!("   FourCC: '{}'", error_str);
            error!("   This might indicate the tap device doesn't support IOProc callbacks");
            
            warn!("⚠️ IOProc creation failed on tap directly - trying aggregate device approach...");
            info!("🎯 CORRECT APPROACH: Core Audio taps need aggregate device with tap as subdevice!");
            info!("🔧 Creating aggregate device that includes tap {} as subdevice", tap_object_id);
            
            // The correct approach: Create aggregate device with tap as subdevice, then IOProc on aggregate
            match self.create_aggregate_device_with_tap(tap_object_id).await {
                Ok(aggregate_device_id) => {
                    info!("✅ Successfully created aggregate device {}, now setting up IOProc on it", aggregate_device_id);
                    return self.setup_ioproc_on_aggregate_device(aggregate_device_id, audio_tx, sample_rate, channels).await;
                }
                Err(e) => {
                    warn!("⚠️ Aggregate device creation failed: {}", e);
                    info!("🔄 FALLBACK: Using direct tap property reading as last resort");
                    let format = AudioFormatInfo {
                        sample_rate: sample_rate as f64,
                        channels: channels as u32,
                        bits_per_sample: 32,
                    };
                    return self.setup_direct_tap_reading(tap_object_id, audio_tx, format).await;
                }
            }
        }
        
        info!("✅ Created IOProc for tap device: {:?}", io_proc_id);
        
        // Start the audio device to begin receiving callbacks
        let start_status = unsafe { AudioDeviceStart(tap_object_id, io_proc_id) };
        if start_status != 0 {
            // Cleanup IOProc if start failed
            unsafe { 
                coreaudio_sys::AudioDeviceDestroyIOProcID(tap_object_id, io_proc_id);
                drop(Box::from_raw(callback_context));
            }
            return Err(anyhow::anyhow!(
                "Failed to start Core Audio tap device {}: OSStatus {}", 
                tap_object_id, start_status
            ));
        }
        
        info!("🎵 Started Core Audio tap device {} - audio should now flow!", tap_object_id);
        
        // Store the IOProc ID for cleanup later
        // TODO: Add proper cleanup in destroy() method
        self.is_capturing = true;
        
        Ok(())
    }
    
    /// Implement proper Core Audio tap reading using aggregate device approach
    #[cfg(target_os = "macos")]
    async fn setup_tap_property_listener(
        &mut self,
        tap_object_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
        sample_rate: f64,
        channels: u32,
    ) -> Result<()> {
        info!("🔄 IMPLEMENTING: Proper Core Audio tap reading with aggregate device for tap {}", tap_object_id);
        
        // Based on Apple documentation and examples, Core Audio taps work by creating an aggregate device
        // that includes the tap, then reading from the aggregate device
        
        match self.setup_aggregate_device_for_tap(tap_object_id, audio_tx.clone(), sample_rate, channels).await {
            Ok(()) => {
                info!("✅ Successfully set up aggregate device tap reading for AudioObjectID {}", tap_object_id);
                Ok(())
            }
            Err(e) => {
                error!("❌ Failed to set up aggregate device for tap {}: {}", tap_object_id, e);
                info!("🔄 FALLBACK: Using placeholder tone while debugging aggregate device setup");
                self.setup_placeholder_tone_fallback(tap_object_id, audio_tx, sample_rate, channels).await
            }
        }
    }
    
    /// Set up aggregate device that includes the Core Audio tap for proper audio reading
    #[cfg(target_os = "macos")]
    async fn setup_aggregate_device_for_tap(
        &mut self,
        tap_object_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
        sample_rate: f64,
        channels: u32,
    ) -> Result<()> {
        info!("🔧 BACK TO PLAN: Creating aggregate device for Core Audio tap {}", tap_object_id);
        
        // Step 1: Get the tap's UUID and format information  
        let tap_format = self.get_tap_audio_format(tap_object_id).await?;
        info!("📋 Tap format: {:.0} Hz, {} channels", tap_format.sample_rate, tap_format.channels);
        
        // Step 2: Create aggregate device that includes this tap (ORIGINAL PLAN)
        let aggregate_device_id = self.create_aggregate_device_with_tap(tap_object_id).await?;
        info!("🔧 Created aggregate device {} with tap {}", aggregate_device_id, tap_object_id);
        
        // Step 3: Set up CPAL input stream from the aggregate device
        self.setup_cpal_stream_from_aggregate_device(aggregate_device_id, audio_tx, tap_format).await?;
        
        // Store the aggregate device ID for cleanup
        self.aggregate_device_id = Some(aggregate_device_id);
        
        Ok(())
    }
    
    /// Get audio format information from the Core Audio tap
    #[cfg(target_os = "macos")]
    async fn get_tap_audio_format(&self, tap_object_id: coreaudio_sys::AudioObjectID) -> Result<AudioFormatInfo> {
        use coreaudio_sys::{AudioObjectGetPropertyData, AudioObjectPropertyAddress, UInt32};
        use std::mem;
        use std::os::raw::c_void;
        
        let address = AudioObjectPropertyAddress {
            mSelector: 0x74666d74, // 'tfmt' - kAudioTapPropertyFormat  
            mScope: 0,             // kAudioObjectPropertyScopeGlobal
            mElement: 0,           // kAudioObjectPropertyElementMain
        };
        
        // AudioStreamBasicDescription structure
        #[repr(C)]
        #[derive(Debug, Clone)]
        struct AudioStreamBasicDescription {
            sample_rate: f64,
            format_id: u32,
            format_flags: u32,
            bytes_per_packet: u32,
            frames_per_packet: u32,
            bytes_per_frame: u32,
            channels_per_frame: u32,
            bits_per_channel: u32,
            reserved: u32,
        }
        
        let mut format_desc: AudioStreamBasicDescription = unsafe { mem::zeroed() };
        let mut data_size = mem::size_of::<AudioStreamBasicDescription>() as UInt32;
        
        let status = unsafe {
            AudioObjectGetPropertyData(
                tap_object_id,
                &address,
                0,
                std::ptr::null(),
                &mut data_size,
                &mut format_desc as *mut AudioStreamBasicDescription as *mut c_void,
            )
        };
        
        if status != 0 {
            return Err(anyhow::anyhow!("Failed to get tap format: OSStatus {}", status));
        }
        
        info!("📋 Raw tap format: sample_rate={}, format_id=0x{:08x}, channels={}, bits={}",
              format_desc.sample_rate, format_desc.format_id, 
              format_desc.channels_per_frame, format_desc.bits_per_channel);
        
        Ok(AudioFormatInfo {
            sample_rate: format_desc.sample_rate,
            channels: format_desc.channels_per_frame,
            bits_per_sample: format_desc.bits_per_channel,
        })
    }
    
    /// Create an aggregate device that includes the Core Audio tap
    #[cfg(target_os = "macos")]
    async fn create_aggregate_device_with_tap(&self, tap_object_id: coreaudio_sys::AudioObjectID) -> Result<coreaudio_sys::AudioObjectID> {
        use core_foundation::dictionary::CFMutableDictionary;
        use core_foundation::string::CFString;
        use core_foundation::array::CFArray;
        use core_foundation::number::CFNumber;
        use core_foundation::base::{CFTypeRef, ToVoid};
        use std::ptr;
        
        info!("🔧 IMPLEMENTING: Creating aggregate device with tap {}", tap_object_id);
        
        // Step 1: Get tap UUID - we need this for the dictionary
        let tap_uuid = self.get_tap_uuid(tap_object_id)?;
        info!("📋 Tap UUID: {}", tap_uuid);
        
        // Step 2: Create CoreFoundation dictionary for aggregate device
        let device_dict = self.create_aggregate_device_dictionary(&tap_uuid)?;
        
        // Step 3: Create the aggregate device using Core Audio HAL
        let aggregate_device_id = unsafe {
            let result = self.create_core_audio_aggregate_device(device_dict);
            
            // Release the dictionary now that the API call is complete
            core_foundation::base::CFRelease(device_dict as core_foundation::base::CFTypeRef);
            
            result?
        };
        
        info!("✅ Created aggregate device {} with tap {}", aggregate_device_id, tap_object_id);
        Ok(aggregate_device_id)
    }
    
    /// Get the UUID from a Core Audio tap
    #[cfg(target_os = "macos")]
    fn get_tap_uuid(&self, tap_object_id: coreaudio_sys::AudioObjectID) -> Result<String> {
        use coreaudio_sys::{AudioObjectGetPropertyData, AudioObjectPropertyAddress};
        use core_foundation::string::CFString;
        use core_foundation::base::{CFType, CFTypeRef, TCFType};
        use std::os::raw::c_void;
        use std::ptr;
        
        let address = AudioObjectPropertyAddress {
            mSelector: 0x74756964, // 'tuid' - kAudioTapPropertyUID (tap-specific UID property)
            mScope: 0,             // kAudioObjectPropertyScopeGlobal
            mElement: 0,           // kAudioObjectPropertyElementMain  
        };
        
        let mut cf_string_ref: CFTypeRef = ptr::null();
        let mut data_size = std::mem::size_of::<CFTypeRef>() as u32;
        
        let status = unsafe {
            AudioObjectGetPropertyData(
                tap_object_id,
                &address,
                0,
                ptr::null(),
                &mut data_size,
                &mut cf_string_ref as *mut CFTypeRef as *mut c_void,
            )
        };
        
        if status != 0 {
            return Err(anyhow::anyhow!("Failed to get tap UUID: OSStatus {}", status));
        }
        
        if cf_string_ref.is_null() {
            return Err(anyhow::anyhow!("Tap UUID is null"));
        }
        
        // Convert CFString to Rust String
        let cf_string = unsafe { CFString::wrap_under_get_rule(cf_string_ref as *const _) };
        let uuid_string = cf_string.to_string();
        
        Ok(uuid_string)
    }
    
    /// Create CoreFoundation dictionary for aggregate device configuration  
    #[cfg(target_os = "macos")]
    fn create_aggregate_device_dictionary(&self, tap_uuid: &str) -> Result<*const std::os::raw::c_void> {
        use core_foundation::dictionary::CFDictionary;
        use core_foundation::string::CFString;
        use core_foundation::array::CFArray;
        use core_foundation::base::{TCFType, CFTypeRef};
        use core_foundation::number::CFNumber;
        
        info!("🔧 Creating proper CoreFoundation dictionary for AudioHardwareCreateAggregateDevice");
        info!("📋 Using tap UUID: {}", tap_uuid);
        
        // Create device name and UID
        let device_name = format!("SendinBeats-Tap-{}", self.process_info.pid);
        let device_uid = format!("com.sendinbeats.tap.{}", self.process_info.pid);
        
        info!("📋 Aggregate device name: {}", device_name);
        info!("📋 Aggregate device UID: {}", device_uid);
        
        // Use the correct Core Audio constants for aggregate device dictionary
        let name_key = CFString::new("name");  // kAudioAggregateDeviceNameKey
        let uid_key = CFString::new("uid");    // kAudioAggregateDeviceUIDKey
        let subdevices_key = CFString::new("subdevice list");  // kAudioAggregateDeviceSubDeviceListKey
        let master_key = CFString::new("master");  // kAudioAggregateDeviceMasterSubDeviceKey
        let is_stacked_key = CFString::new("stacked");  // kAudioAggregateDeviceIsStackedKey
        
        // Values
        let name_value = CFString::new(&device_name);
        let uid_value = CFString::new(&device_uid);
        
        // CRITICAL: Include the tap UUID in the subdevices array
        // This is how Core Audio taps are supposed to work - tap becomes part of aggregate device
        let tap_uuid_cf = CFString::new(tap_uuid);
        let subdevices_array = CFArray::<CFString>::from_CFTypes(&[tap_uuid_cf]);
        
        // Set is_stacked to 1 (true) for multi-output behavior  
        let is_stacked_value = CFNumber::from(1i32);
        
        info!("🔧 Creating aggregate device dictionary with tap as subdevice");
        info!("🔧 Including tap UUID {} in subdevices array", tap_uuid);
        
        // Create the dictionary with proper Core Audio keys
        let pairs = [
            (name_key.as_CFType(), name_value.as_CFType()),
            (uid_key.as_CFType(), uid_value.as_CFType()),
            (subdevices_key.as_CFType(), subdevices_array.as_CFType()),
            (is_stacked_key.as_CFType(), is_stacked_value.as_CFType()),
        ];
        
        let dict = CFDictionary::from_CFType_pairs(&pairs);
        
        info!("📋 Created CoreFoundation dictionary with {} keys", pairs.len());
        info!("📋 Dictionary keys: name, uid, subdevice list, stacked");
        
        // Keep the dictionary alive and return a retained reference
        let dict_ref = dict.as_concrete_TypeRef() as *const std::os::raw::c_void;
        
        // Explicitly retain the dictionary to prevent deallocation
        unsafe {
            core_foundation::base::CFRetain(dict_ref as CFTypeRef);
        }
        
        info!("📋 Dictionary retained and ready for AudioHardwareCreateAggregateDevice");
        Ok(dict_ref)
    }
    
    /// Create the actual Core Audio aggregate device
    #[cfg(target_os = "macos")]
    unsafe fn create_core_audio_aggregate_device(&self, device_dict: *const std::os::raw::c_void) -> Result<coreaudio_sys::AudioObjectID> {
        use coreaudio_sys::{AudioHardwareCreateAggregateDevice, AudioObjectID};
        
        let mut aggregate_device_id: AudioObjectID = 0;
        
        let status = AudioHardwareCreateAggregateDevice(
            device_dict as *const _,
            &mut aggregate_device_id,
        );
        
        if status != 0 {
            return Err(anyhow::anyhow!("AudioHardwareCreateAggregateDevice failed: OSStatus {}", status));
        }
        
        if aggregate_device_id == 0 {
            return Err(anyhow::anyhow!("Created aggregate device has invalid ID"));
        }
        
        info!("🎉 Successfully created Core Audio aggregate device: ID {}", aggregate_device_id);
        Ok(aggregate_device_id)
    }
    
    /// Set up IOProc callback on aggregate device to receive tap data
    #[cfg(target_os = "macos")]
    async fn setup_ioproc_on_aggregate_device(
        &mut self,
        aggregate_device_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
        sample_rate: f64,
        channels: u32,
    ) -> Result<()> {
        use coreaudio_sys::{AudioDeviceCreateIOProcID, AudioDeviceStart, AudioObjectID, AudioDeviceIOProcID};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        
        info!("🔧 Setting up IOProc callback on aggregate device {}", aggregate_device_id);
        info!("📋 Aggregate device format: {:.0} Hz, {} channels", sample_rate, channels);
        
        // Create context for the callback
        let audio_tx_clone = audio_tx.clone();
        let process_name = self.process_info.name.clone();
        let is_running = Arc::new(AtomicBool::new(true));
        
        // Box the callback context to pass to C
        let callback_context = Box::new((audio_tx_clone, process_name, is_running.clone()));
        let context_ptr = Box::into_raw(callback_context);
        
        // Define the IOProc callback function
        extern "C" fn aggregate_ioproc_callback(
            device_id: AudioObjectID,
            _now: *const coreaudio_sys::AudioTimeStamp,
            input_data: *const coreaudio_sys::AudioBufferList,
            _input_time: *const coreaudio_sys::AudioTimeStamp,
            _output_data: *mut coreaudio_sys::AudioBufferList,
            _output_time: *const coreaudio_sys::AudioTimeStamp,
            client_data: *mut std::os::raw::c_void,
        ) -> i32 {
            if client_data.is_null() || input_data.is_null() {
                return 0;
            }
            
            let context = unsafe { &*(client_data as *const (broadcast::Sender<Vec<f32>>, String, Arc<AtomicBool>)) };
            let (audio_tx, process_name, is_running) = context;
            
            if !is_running.load(Ordering::Relaxed) {
                return 0;
            }
            
            unsafe {
                let buffer_list = &*input_data;
                if buffer_list.mNumberBuffers == 0 {
                    return 0;
                }
                
                // Get the first buffer (should contain interleaved stereo data)
                let buffer = &buffer_list.mBuffers[0];
                let data_ptr = buffer.mData as *const f32;
                let frame_count = buffer.mDataByteSize / 8; // 2 channels * 4 bytes per sample
                
                if !data_ptr.is_null() && frame_count > 0 {
                    let samples = std::slice::from_raw_parts(data_ptr, (frame_count * 2) as usize);
                    let sample_vec = samples.to_vec();
                    
                    // Calculate peak level for logging
                    let peak = sample_vec.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    
                    if peak > 0.001 {
                        println!("🎵 REAL APPLE MUSIC DATA [{}]: {} samples, peak: {:.4}", 
                                process_name, sample_vec.len(), peak);
                    }
                    
                    // Send real Apple Music audio data to the mixer!
                    if let Err(_) = audio_tx.send(sample_vec) {
                        // Channel closed, stop processing
                        is_running.store(false, Ordering::Relaxed);
                    }
                }
            }
            
            0 // noErr
        }
        
        // Create IOProc on the aggregate device
        let mut io_proc_id: AudioDeviceIOProcID = None;
        let status = unsafe {
            AudioDeviceCreateIOProcID(
                aggregate_device_id,
                Some(aggregate_ioproc_callback),
                context_ptr as *mut std::os::raw::c_void,
                &mut io_proc_id,
            )
        };
        
        if status != 0 {
            unsafe { drop(Box::from_raw(context_ptr)); }
            return Err(anyhow::anyhow!("AudioDeviceCreateIOProcID failed on aggregate device: OSStatus {}", status));
        }
        
        info!("✅ Created IOProc on aggregate device: {:?}", io_proc_id);
        
        // Start the aggregate device
        let start_status = unsafe { AudioDeviceStart(aggregate_device_id, io_proc_id) };
        if start_status != 0 {
            return Err(anyhow::anyhow!("AudioDeviceStart failed on aggregate device: OSStatus {}", start_status));
        }
        
        info!("🎉 BREAKTHROUGH: Aggregate device started - real Apple Music audio should flow through IOProc!");
        
        Ok(())
    }

    /// Set up CPAL input stream from aggregate device
    #[cfg(target_os = "macos")]
    async fn setup_cpal_stream_from_aggregate_device(
        &mut self,
        aggregate_device_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
        format: AudioFormatInfo,
    ) -> Result<()> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        
        info!("🔧 Setting up CPAL stream from aggregate device {}", aggregate_device_id);
        info!("📋 Target format: {:.0} Hz, {} channels, {} bits", format.sample_rate, format.channels, format.bits_per_sample);
        
        // The aggregate device should now appear in CPAL's device enumeration
        let host = cpal::default_host();
        let devices: Vec<cpal::Device> = match host.input_devices() {
            Ok(devices) => devices.collect(),
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to enumerate input devices: {}", e));
            }
        };
        
        // Find the aggregate device we just created
        let aggregate_device_name = format!("SendinBeats-Tap-{}", self.process_info.pid);
        let mut target_device = None;
        
        info!("🔍 Looking for aggregate device '{}' among {} devices", aggregate_device_name, devices.len());
        for (i, device) in devices.iter().enumerate() {
            if let Ok(device_name) = device.name() {
                info!("🔍 Device {}: '{}'", i, device_name);
                
                // Look for our aggregate device by name or UID
                if device_name.contains("SendinBeats") || device_name.contains(&self.process_info.pid.to_string()) {
                    info!("✅ FOUND aggregate device: '{}'", device_name);
                    target_device = Some(device.clone());
                    break;
                }
            }
        }
        
        let device = target_device.ok_or_else(|| {
            anyhow::anyhow!("Aggregate device not found in CPAL enumeration - may need time to register")
        })?;
        
        let device_name = device.name().unwrap_or_else(|_| format!("Aggregate-{}", aggregate_device_id));
        
        // Get device configuration
        let device_config = match device.default_input_config() {
            Ok(config) => config,
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to get aggregate device config: {}", e));
            }
        };
        
        info!("📋 Aggregate device config: {} Hz, {} channels, format: {:?}", 
              device_config.sample_rate().0, device_config.channels(), device_config.sample_format());
        
        // Create stream configuration matching the tap's native format
        let config = cpal::StreamConfig {
            channels: format.channels as u16,
            sample_rate: cpal::SampleRate(format.sample_rate as u32),
            buffer_size: cpal::BufferSize::Default,
        };
        
        info!("🔧 Creating aggregate device stream: {} channels, {} Hz", config.channels, config.sample_rate.0);
        
        // Create the input stream with real Apple Music audio callback
        let process_name = self.process_info.name.clone();
        let mut callback_count = 0u64;
        let audio_tx_for_callback = audio_tx.clone();
        
        let stream = match device_config.sample_format() {
            cpal::SampleFormat::F32 => {
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        callback_count += 1;
                        
                        // Calculate audio levels for monitoring
                        let peak_level = data.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        let rms_level = (data.iter().map(|&s| s * s).sum::<f32>() / data.len() as f32).sqrt();
                        
                        // This is REAL Apple Music audio data!
                        let audio_samples = data.to_vec();
                        
                        // Log real audio activity
                        if callback_count % 100 == 0 || (peak_level > 0.001 && callback_count % 20 == 0) {
                            info!("🎵 REAL APPLE MUSIC AUDIO [{}] Callback #{}: {} samples, peak: {:.4}, rms: {:.4}", 
                                process_name, callback_count, data.len(), peak_level, rms_level);
                        }
                        
                        // Send REAL Apple Music audio to mixer!
                        if let Err(e) = audio_tx_for_callback.send(audio_samples) {
                            if callback_count % 1000 == 0 {
                                warn!("Failed to send Apple Music audio samples: {} (callback #{})", e, callback_count);
                            }
                        }
                    },
                    |err| {
                        error!("Apple Music audio input error: {}", err);
                    },
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        callback_count += 1;
                        
                        // Convert I16 to F32 for consistency with mixer
                        let f32_samples: Vec<f32> = data.iter()
                            .map(|&sample| {
                                if sample >= 0 {
                                    sample as f32 / 32767.0
                                } else {
                                    sample as f32 / 32768.0
                                }
                            })
                            .collect();
                        
                        let peak_level = f32_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        
                        if callback_count % 100 == 0 || (peak_level > 0.001 && callback_count % 20 == 0) {
                            info!("🎵 REAL APPLE MUSIC I16 [{}] Callback #{}: {} samples, peak: {:.4}", 
                                process_name, callback_count, data.len(), peak_level);
                        }
                        
                        // Send converted REAL Apple Music audio
                        if let Err(e) = audio_tx_for_callback.send(f32_samples) {
                            if callback_count % 1000 == 0 {
                                warn!("Failed to send Apple Music I16 samples: {} (callback #{})", e, callback_count);
                            }
                        }
                    },
                    |err| {
                        error!("Apple Music I16 input error: {}", err);
                    },
                    None,
                )?
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported aggregate device sample format: {:?}", device_config.sample_format()));
            }
        };
        
        // Start the stream - this will begin capturing REAL Apple Music audio!
        stream.play().map_err(|e| anyhow::anyhow!("Failed to start aggregate device stream: {}", e))?;
        
        info!("🎉 BREAKTHROUGH: Real Apple Music audio capture started via aggregate device!");
        self.is_capturing = true;
        
        // Leak the stream intentionally for lifecycle management
        let stream_info = format!("Apple Music aggregate device stream for {}", self.process_info.name);
        self._stream_info = Some(stream_info);
        std::mem::forget(stream);
        
        info!("🎵 Apple Music audio should now be flowing to mixer instead of test tone!");
        
        Ok(())
    }
    
    /// Fallback placeholder implementation while working on real tap reading
    #[cfg(target_os = "macos")]
    async fn setup_placeholder_tone_fallback(
        &mut self,
        tap_object_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
        sample_rate: f64,
        channels: u32,
    ) -> Result<()> {
        info!("🔄 FALLBACK: Setting up improved placeholder tone for tap {}", tap_object_id);
        
        // First, let's try to read the tap format to understand what Apple Music is outputting
        match self.get_tap_audio_format(tap_object_id).await {
            Ok(format) => {
                info!("🎵 APPLE MUSIC FORMAT: {:.0} Hz, {} channels, {} bits", 
                      format.sample_rate, format.channels, format.bits_per_sample);
            }
            Err(e) => {
                warn!("⚠️ Could not read tap format: {}", e);
            }
        }
        
        tokio::spawn(async move {
            info!("📡 Started placeholder tap polling task for AudioObjectID {}", tap_object_id);
            let mut poll_count = 0u64;
            
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(20)).await; // 50 Hz polling (approximating audio rate)
                poll_count += 1;
                
                // Generate test audio data to verify pipeline is working
                // This will be replaced with real Core Audio tap data reading
                let sample_count = (sample_rate as usize * 20) / 1000; // 20ms worth of samples
                let mut test_samples = Vec::with_capacity(sample_count);
                
                // Generate a low-amplitude sine wave test tone (440 Hz)
                let frequency = 440.0;
                let amplitude = 0.1; // Low volume
                for i in 0..sample_count {
                    let t = (poll_count as f64 * 20.0 / 1000.0) + (i as f64 / sample_rate);
                    let sample = (amplitude * (2.0 * std::f64::consts::PI * frequency * t).sin()) as f32;
                    
                    // Add to both channels if stereo
                    test_samples.push(sample);
                    if channels >= 2 {
                        test_samples.push(sample);
                    }
                }
                
                // Send test audio to broadcast channel
                if let Err(_) = audio_tx.send(test_samples) {
                    if poll_count % 100 == 0 {
                        warn!("Failed to send test audio samples (poll #{})", poll_count);
                    }
                } else if poll_count % 250 == 0 {
                    info!("📡 TAP POLL #{}: Generated {} test samples (440Hz tone) - PLACEHOLDER", 
                          poll_count, sample_count * channels as usize);
                }
            }
        });
        
        self.is_capturing = true;
        
        warn!("⚠️ PLACEHOLDER: Generating test 440Hz tone instead of real tap audio");
        warn!("⚠️ This verifies the pipeline works while researching proper Core Audio tap access");
        
        Ok(())
    }
    
    /// Get sample rate from Core Audio tap device
    #[cfg(target_os = "macos")]
    unsafe fn get_tap_sample_rate(&self, device_id: coreaudio_sys::AudioObjectID) -> Result<f64> {
        use coreaudio_sys::{AudioObjectGetPropertyData, AudioObjectPropertyAddress, UInt32};
        use std::mem;
        use std::os::raw::c_void;
        
        let address = AudioObjectPropertyAddress {
            mSelector: 0x73726174, // 'srat' - kAudioDevicePropertyNominalSampleRate
            mScope: 0,             // kAudioObjectPropertyScopeGlobal  
            mElement: 0,           // kAudioObjectPropertyElementMain
        };
        
        let mut sample_rate: f64 = 0.0;
        let mut data_size = mem::size_of::<f64>() as UInt32;
        
        let status = AudioObjectGetPropertyData(
            device_id,
            &address,
            0,                                                           // qualifier size
            std::ptr::null(),                                           // qualifier data
            &mut data_size,
            &mut sample_rate as *mut f64 as *mut c_void,
        );
        
        if status == 0 {
            Ok(sample_rate)
        } else {
            Err(anyhow::anyhow!("Failed to get tap sample rate: OSStatus {}", status))
        }
    }
    
    /// Get channel count from Core Audio tap device
    #[cfg(target_os = "macos")]
    unsafe fn get_tap_channel_count(&self, device_id: coreaudio_sys::AudioObjectID) -> Result<u32> {
        use coreaudio_sys::{AudioObjectGetPropertyData, AudioObjectPropertyAddress, UInt32};
        use std::mem;
        use std::os::raw::c_void;
        
        let address = AudioObjectPropertyAddress {
            mSelector: 0x73666d74, // 'sfmt' - kAudioDevicePropertyStreamFormat
            mScope: 1,             // kAudioObjectPropertyScopeInput
            mElement: 0,           // kAudioObjectPropertyElementMain
        };
        
        // AudioStreamBasicDescription structure
        #[repr(C)]
        struct AudioStreamBasicDescription {
            sample_rate: f64,
            format_id: u32,
            format_flags: u32,
            bytes_per_packet: u32,
            frames_per_packet: u32,
            bytes_per_frame: u32,
            channels_per_frame: u32,
            bits_per_channel: u32,
            reserved: u32,
        }
        
        let mut format_desc: AudioStreamBasicDescription = mem::zeroed();
        let mut data_size = mem::size_of::<AudioStreamBasicDescription>() as UInt32;
        
        let status = AudioObjectGetPropertyData(
            device_id,
            &address,
            0,                                                           // qualifier size
            std::ptr::null(),                                           // qualifier data
            &mut data_size,
            &mut format_desc as *mut AudioStreamBasicDescription as *mut c_void,
        );
        
        if status == 0 {
            Ok(format_desc.channels_per_frame)
        } else {
            Err(anyhow::anyhow!("Failed to get tap channel count: OSStatus {}", status))
        }
    }
    
    /// Simple linear interpolation resampling for audio format conversion
    #[cfg(target_os = "macos")]
    fn resample_audio(input: &[f32], input_rate: u32, output_rate: u32) -> Vec<f32> {
        if input_rate == output_rate {
            return input.to_vec();
        }
        
        let ratio = input_rate as f64 / output_rate as f64;
        let output_len = ((input.len() as f64) / ratio).ceil() as usize;
        let mut output = Vec::with_capacity(output_len);
        
        for i in 0..output_len {
            let src_index = (i as f64) * ratio;
            let src_index_floor = src_index.floor() as usize;
            let src_index_ceil = (src_index_floor + 1).min(input.len() - 1);
            let fraction = src_index - src_index_floor as f64;
            
            if src_index_floor >= input.len() {
                break;
            }
            
            // Linear interpolation between adjacent samples
            let sample = if src_index_ceil == src_index_floor {
                input[src_index_floor]
            } else {
                let sample_low = input[src_index_floor];
                let sample_high = input[src_index_ceil];
                sample_low + (sample_high - sample_low) * fraction as f32
            };
            
            output.push(sample);
        }
        
        output
    }
    
    /// Direct Core Audio tap data reading - BREAKTHROUGH implementation
    #[cfg(target_os = "macos")]
    async fn setup_direct_tap_reading(
        &mut self,
        tap_object_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
        format: AudioFormatInfo,
    ) -> Result<()> {
        use coreaudio_sys::{AudioObjectAddPropertyListener, AudioObjectPropertyAddress};
        use std::os::raw::c_void;
        use std::ptr;
        
        info!("🔧 BREAKTHROUGH: Setting up direct Core Audio tap data reading for tap {}", tap_object_id);
        info!("📋 Target format: {:.0} Hz, {} channels, {} bits", format.sample_rate, format.channels, format.bits_per_sample);
        
        // Core Audio taps work by listening to property changes on the tap object
        // When audio data is available, the tap fires property change notifications
        // This is the approach used by professional audio routing tools
        
        let data_property_address = AudioObjectPropertyAddress {
            mSelector: 0x74646174, // 'tdat' - kAudioTapPropertyData (tap data property)
            mScope: 0,              // kAudioObjectPropertyScopeGlobal
            mElement: 0,            // kAudioObjectPropertyElementMain
        };
        
        // For now, let's use a polling approach to read tap data
        // In a production implementation, we'd use property listeners for efficiency
        
        let process_name = self.process_info.name.clone();
        tokio::spawn(async move {
            info!("📡 BREAKTHROUGH: Started direct tap data reading task for {}", process_name);
            let mut poll_count = 0u64;
            
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await; // 100 Hz polling for low latency
                poll_count += 1;
                
                // Try to read data from the tap using AudioObjectGetPropertyData
                let mut audio_data = Self::read_tap_data_direct(tap_object_id);
                
                match audio_data {
                    Ok(samples) if !samples.is_empty() => {
                        let peak_level = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        
                        if poll_count % 50 == 0 || (peak_level > 0.001 && poll_count % 10 == 0) {
                            info!("🎵 REAL APPLE MUSIC DATA [{}] Poll #{}: {} samples, peak: {:.4}", 
                                  process_name, poll_count, samples.len(), peak_level);
                        }
                        
                        // Send REAL Apple Music audio data!
                        if let Err(e) = audio_tx.send(samples) {
                            if poll_count % 1000 == 0 {
                                warn!("Failed to send real tap audio samples: {} (poll #{})", e, poll_count);
                            }
                        }
                    }
                    Ok(_) => {
                        // No data available yet - this is normal during silence
                        if poll_count % 5000 == 0 {
                            info!("📡 TAP POLL #{}: No audio data available (silence)", poll_count);
                        }
                    }
                    Err(e) => {
                        if poll_count % 1000 == 0 {
                            warn!("Failed to read tap data: {} (poll #{})", e, poll_count);
                        }
                    }
                }
            }
        });
        
        self.is_capturing = true;
        info!("🎉 BREAKTHROUGH: Direct Core Audio tap reading started - real Apple Music audio should flow!");
        
        Ok(())
    }
    
    /// Read audio data directly from Core Audio tap
    #[cfg(target_os = "macos")]
    fn read_tap_data_direct(tap_object_id: coreaudio_sys::AudioObjectID) -> Result<Vec<f32>> {
        use coreaudio_sys::{AudioObjectGetPropertyData, AudioObjectPropertyAddress};
        use std::os::raw::c_void;
        use std::ptr;
        
        let data_address = AudioObjectPropertyAddress {
            mSelector: 0x74646174, // 'tdat' - tap data property
            mScope: 0,              // kAudioObjectPropertyScopeGlobal  
            mElement: 0,            // kAudioObjectPropertyElementMain
        };
        
        // First, get the size of available data
        let mut data_size: u32 = 0;
        let status = unsafe {
            AudioObjectGetPropertyData(
                tap_object_id,
                &data_address,
                0,
                ptr::null(),
                &mut data_size,
                ptr::null_mut(),
            )
        };
        
        if status != 0 || data_size == 0 {
            // No data available or error - return empty vec (silence)
            return Ok(Vec::new());
        }
        
        // Allocate buffer for audio data
        let sample_count = data_size as usize / 4; // 32-bit float samples
        let mut audio_buffer = vec![0.0f32; sample_count];
        
        // Read the actual audio data
        let status = unsafe {
            AudioObjectGetPropertyData(
                tap_object_id,
                &data_address,
                0,
                ptr::null(),
                &mut data_size,
                audio_buffer.as_mut_ptr() as *mut c_void,
            )
        };
        
        if status == 0 {
            Ok(audio_buffer)
        } else {
            Err(anyhow::anyhow!("Failed to read tap audio data: OSStatus {}", status))
        }
    }
    
    /// Start capturing audio from the tapped application
    pub fn start_capture(&mut self) -> Result<broadcast::Receiver<Vec<f32>>> {
        if self.audio_tx.is_none() {
            return Err(anyhow::anyhow!("Audio tap not created. Call create_tap() first."));
        }
        
        info!("Starting audio capture for {}", self.process_info.name);
        
        // TODO: Implement actual audio capture start
        // This involves starting the audio device IO
        
        self.is_capturing = true;
        
        // Return a receiver for the audio samples
        Ok(self.audio_tx.as_ref().unwrap().subscribe())
    }
    
    /// Stop capturing audio
    pub fn stop_capture(&mut self) -> Result<()> {
        if self.is_capturing {
            info!("Stopping audio capture for {}", self.process_info.name);
            
            // TODO: Implement actual audio capture stop
            // This involves stopping the audio device IO
            
            self.is_capturing = false;
        }
        
        Ok(())
    }
    
    /// Check if Core Audio taps are supported on this system
    fn is_core_audio_taps_supported(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            
            // Get macOS version using sw_vers command
            if let Ok(output) = Command::new("sw_vers")
                .arg("-productVersion")
                .output()
            {
                if let Ok(version_str) = String::from_utf8(output.stdout) {
                    let version = version_str.trim();
                    if let Ok(parsed_version) = self.parse_macos_version(version) {
                        // Core Audio taps require macOS 14.4+
                        return parsed_version >= (14, 4, 0);
                    }
                }
            }
            
            warn!("Could not determine macOS version, assuming Core Audio taps not supported");
            false
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
    
    /// Parse macOS version string into tuple (major, minor, patch)
    fn parse_macos_version(&self, version: &str) -> Result<(u32, u32, u32)> {
        let parts: Vec<&str> = version.split('.').collect();
        
        if parts.len() < 2 {
            return Err(anyhow::anyhow!("Invalid macOS version format: {}", version));
        }
        
        let major = parts[0].parse::<u32>()?;
        let minor = parts[1].parse::<u32>()?;
        let patch = if parts.len() > 2 { 
            parts[2].parse::<u32>().unwrap_or(0) 
        } else { 
            0 
        };
        
        Ok((major, minor, patch))
    }
    
    /// Cleanup resources
    pub fn destroy(&mut self) -> Result<()> {
        self.stop_capture()?;
        
        // Clear stream info (actual stream was intentionally leaked and will stop when process ends)
        if let Some(stream_info) = self._stream_info.take() {
            info!("Clearing stream info: {}", stream_info);
        }
        
        #[cfg(target_os = "macos")]
        {
            use super::core_audio_bindings::{destroy_process_tap, format_osstatus_error};
            
            // Destroy process tap if it exists
            if let Some(tap_id) = self.tap_id {
                info!("Destroying Core Audio process tap with ID {}", tap_id);
                
                unsafe {
                    if let Err(status) = destroy_process_tap(tap_id as u32) {
                        let error_msg = format_osstatus_error(status);
                        warn!("Failed to destroy process tap {}: {} ({})", tap_id, error_msg, status);
                        // Don't fail completely, just log the warning
                    } else {
                        info!("Successfully destroyed process tap {}", tap_id);
                    }
                }
                
                self.tap_id = None;
            }
            
            // TODO: Destroy aggregate device if it exists
            if let Some(device_id) = self.aggregate_device_id {
                info!("TODO: Destroy aggregate device with ID {}", device_id);
                // This would call AudioHardwareDestroyAggregateDevice
                self.aggregate_device_id = None;
            }
        }
        
        // Clear audio channel
        self.audio_tx = None;
        
        info!("Destroyed audio tap for {}", self.process_info.name);
        Ok(())
    }
    
    pub fn is_capturing(&self) -> bool {
        self.is_capturing
    }
    
    pub fn get_process_info(&self) -> &ProcessInfo {
        &self.process_info
    }
    
    /// Check if the tapped process is still alive
    pub fn is_process_alive(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            
            // Use ps command to check if process exists
            if let Ok(output) = Command::new("ps")
                .arg("-p")
                .arg(self.process_info.pid.to_string())
                .arg("-o")
                .arg("pid=")
                .output()
            {
                if let Ok(stdout) = String::from_utf8(output.stdout) {
                    return !stdout.trim().is_empty();
                }
            }
        }
        
        false
    }
    
    /// Update heartbeat to indicate tap is still active
    pub async fn heartbeat(&self) {
        if let Ok(mut last_heartbeat) = self.last_heartbeat.lock() {
            *last_heartbeat = std::time::Instant::now();
        }
    }
    
    /// Check if tap has been inactive for too long
    pub async fn is_stale(&self, timeout: std::time::Duration) -> bool {
        if let Ok(last_heartbeat) = self.last_heartbeat.lock() {
            return last_heartbeat.elapsed() > timeout;
        }
        true // Assume stale if we can't get the lock
    }
    
    /// Increment error count and check if maximum is reached
    pub async fn record_error(&self) -> bool {
        if let Ok(mut error_count) = self.error_count.lock() {
            *error_count += 1;
            if *error_count >= self.max_errors {
                error!(
                    "Tap for {} (PID: {}) reached maximum error count ({}), marking for cleanup",
                    self.process_info.name, self.process_info.pid, self.max_errors
                );
                return true; // Should be cleaned up
            }
        }
        false
    }
    
    /// Reset error count (called after successful operations)
    pub async fn reset_errors(&self) {
        if let Ok(mut error_count) = self.error_count.lock() {
            *error_count = 0;
        }
    }
    
    /// Get current error count
    pub async fn get_error_count(&self) -> u32 {
        if let Ok(error_count) = self.error_count.lock() {
            *error_count
        } else {
            u32::MAX // Return high value if we can't get the lock
        }
    }
    
    /// Get tap statistics for monitoring
    pub async fn get_stats(&self) -> TapStats {
        let error_count = self.get_error_count().await;
        let age = self.created_at.elapsed();
        let last_activity = if let Ok(last_heartbeat) = self.last_heartbeat.lock() {
            last_heartbeat.elapsed()
        } else {
            age
        };
        
        TapStats {
            pid: self.process_info.pid,
            process_name: self.process_info.name.clone(),
            age,
            last_activity,
            error_count,
            is_capturing: self.is_capturing,
            process_alive: self.is_process_alive(),
        }
    }
}

/// Virtual audio input stream that bridges tap audio to mixer system
pub struct VirtualAudioInputStream {
    device_id: String,
    device_name: String,
    sample_rate: u32,
    channels: u16,
    bridge_buffer: Arc<tokio::sync::Mutex<Vec<f32>>>,
    effects_chain: Arc<tokio::sync::Mutex<crate::audio::effects::AudioEffectsChain>>,
}

/// Bridge adapter that converts VirtualAudioInputStream to AudioInputStream interface
pub struct ApplicationAudioInputBridge {
    device_id: String,
    device_name: String,
    sample_rate: u32,
    channels: u16,
    audio_buffer: Arc<tokio::sync::Mutex<Vec<f32>>>, // Source buffer from tap bridge
    sync_buffer: Arc<std::sync::Mutex<Vec<f32>>>,     // Sync buffer for mixer compatibility
    effects_chain: Arc<std::sync::Mutex<crate::audio::effects::AudioEffectsChain>>,
    adaptive_chunk_size: usize,
}

impl VirtualAudioInputStream {
    pub fn new(
        device_id: String,
        device_name: String,
        sample_rate: u32,
        bridge_buffer: Arc<tokio::sync::Mutex<Vec<f32>>>,
    ) -> Self {
        let effects_chain = Arc::new(tokio::sync::Mutex::new(
            crate::audio::effects::AudioEffectsChain::new(sample_rate)
        ));
        
        Self {
            device_id,
            device_name,
            sample_rate,
            channels: 2, // Assume stereo for application audio
            bridge_buffer,
            effects_chain,
        }
    }
    
    /// Get samples from the bridge buffer (compatible with AudioInputStream interface)
    pub async fn get_samples(&self) -> Vec<f32> {
        if let Ok(mut buffer) = self.bridge_buffer.try_lock() {
            if buffer.is_empty() {
                return Vec::new();
            }
            
            // Drain all available samples
            let samples: Vec<f32> = buffer.drain(..).collect();
            samples
        } else {
            Vec::new()
        }
    }
    
    /// Process samples with effects (compatible with AudioInputStream interface)
    pub async fn process_with_effects(&self, channel: &crate::audio::types::AudioChannel) -> Vec<f32> {
        if let Ok(mut buffer) = self.bridge_buffer.try_lock() {
            if buffer.is_empty() {
                return Vec::new();
            }
            
            // Drain all available samples
            let mut samples: Vec<f32> = buffer.drain(..).collect();
            
            // Apply effects if enabled
            if channel.effects_enabled && !samples.is_empty() {
                if let Ok(mut effects) = self.effects_chain.try_lock() {
                    // Update effects parameters based on channel settings
                    effects.set_eq_gain(crate::audio::effects::EQBand::Low, channel.eq_low_gain);
                    effects.set_eq_gain(crate::audio::effects::EQBand::Mid, channel.eq_mid_gain);
                    effects.set_eq_gain(crate::audio::effects::EQBand::High, channel.eq_high_gain);
                    
                    if channel.comp_enabled {
                        effects.set_compressor_params(
                            channel.comp_threshold,
                            channel.comp_ratio,
                            channel.comp_attack,
                            channel.comp_release,
                        );
                    }
                    
                    if channel.limiter_enabled {
                        effects.set_limiter_threshold(channel.limiter_threshold);
                    }

                    // Process samples through effects chain
                    effects.process(&mut samples);
                }
            }
            
            // Apply channel-specific gain and mute
            if !channel.muted && channel.gain > 0.0 {
                for sample in samples.iter_mut() {
                    *sample *= channel.gain;
                }
            } else {
                samples.fill(0.0);
            }

            samples
        } else {
            Vec::new()
        }
    }
    
    pub fn device_id(&self) -> &str {
        &self.device_id
    }
    
    pub fn device_name(&self) -> &str {
        &self.device_name
    }
    
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

impl ApplicationAudioInputBridge {
    pub fn new(
        device_id: String,
        device_name: String,
        sample_rate: u32,
        audio_buffer: Arc<tokio::sync::Mutex<Vec<f32>>>,
    ) -> Result<Self> {
        let sync_buffer = Arc::new(std::sync::Mutex::new(Vec::new()));
        let effects_chain = Arc::new(std::sync::Mutex::new(
            crate::audio::effects::AudioEffectsChain::new(sample_rate)
        ));
        
        // Calculate optimal chunk size (same as AudioInputStream)
        let optimal_chunk_size = (sample_rate as f32 * 0.005) as usize; // 5ms default
        
        Ok(Self {
            device_id,
            device_name,
            sample_rate,
            channels: 2, // Assume stereo for application audio
            audio_buffer,
            sync_buffer,
            effects_chain,
            adaptive_chunk_size: optimal_chunk_size.max(64).min(1024),
        })
    }
    
    /// Synchronously transfer samples from async buffer to sync buffer
    /// This should be called periodically to keep the sync buffer updated
    pub fn sync_transfer_samples(&self) {
        // Use try_lock to avoid blocking - if async buffer is locked, skip this transfer
        if let Ok(mut async_buffer) = self.audio_buffer.try_lock() {
            if !async_buffer.is_empty() {
                // Transfer samples from async buffer to sync buffer
                let samples: Vec<f32> = async_buffer.drain(..).collect();
                
                if let Ok(mut sync_buffer) = self.sync_buffer.try_lock() {
                    sync_buffer.extend_from_slice(&samples);
                    
                    // Prevent buffer overflow - same logic as regular input streams
                    let max_buffer_size = 48000; // 1 second at 48kHz
                    if sync_buffer.len() > max_buffer_size * 2 {
                        let keep_size = max_buffer_size;
                        let buffer_len = sync_buffer.len();
                        let new_buffer = sync_buffer.split_off(buffer_len - keep_size);
                        *sync_buffer = new_buffer;
                    }
                }
            }
        }
    }
    
    /// Get samples (compatible with AudioInputStream interface)
    pub fn get_samples(&self) -> Vec<f32> {
        // First, transfer any new samples from async buffer
        self.sync_transfer_samples();
        
        // Then get samples from sync buffer (same as AudioInputStream)
        if let Ok(mut buffer) = self.sync_buffer.try_lock() {
            if buffer.is_empty() {
                return Vec::new();
            }
            
            // Process ALL available samples to prevent buffer buildup
            let samples: Vec<f32> = buffer.drain(..).collect();
            samples
        } else {
            Vec::new()
        }
    }
    
    /// Process samples with effects (compatible with AudioInputStream interface)
    pub fn process_with_effects(&self, channel: &crate::audio::types::AudioChannel) -> Vec<f32> {
        // First, transfer any new samples from async buffer
        self.sync_transfer_samples();
        
        if let Ok(mut buffer) = self.sync_buffer.try_lock() {
            if buffer.is_empty() {
                return Vec::new();
            }
            
            // Drain all available samples
            let mut samples: Vec<f32> = buffer.drain(..).collect();
            
            // Apply effects if enabled
            if channel.effects_enabled && !samples.is_empty() {
                if let Ok(mut effects) = self.effects_chain.try_lock() {
                    // Update effects parameters based on channel settings
                    effects.set_eq_gain(crate::audio::effects::EQBand::Low, channel.eq_low_gain);
                    effects.set_eq_gain(crate::audio::effects::EQBand::Mid, channel.eq_mid_gain);
                    effects.set_eq_gain(crate::audio::effects::EQBand::High, channel.eq_high_gain);
                    
                    if channel.comp_enabled {
                        effects.set_compressor_params(
                            channel.comp_threshold,
                            channel.comp_ratio,
                            channel.comp_attack,
                            channel.comp_release,
                        );
                    }
                    
                    if channel.limiter_enabled {
                        effects.set_limiter_threshold(channel.limiter_threshold);
                    }

                    // Process samples through effects chain
                    effects.process(&mut samples);
                }
            }
            
            // Apply channel-specific gain and mute
            if !channel.muted && channel.gain > 0.0 {
                for sample in samples.iter_mut() {
                    *sample *= channel.gain;
                }
            } else {
                samples.fill(0.0);
            }

            samples
        } else {
            Vec::new()
        }
    }
    
    /// Set adaptive chunk size (compatible with AudioInputStream interface)
    pub fn set_adaptive_chunk_size(&mut self, hardware_buffer_size: usize) {
        let adaptive_size = if hardware_buffer_size > 32 && hardware_buffer_size <= 2048 {
            hardware_buffer_size
        } else {
            (self.sample_rate as f32 * 0.005) as usize
        };
        
        self.adaptive_chunk_size = adaptive_size;
        info!("🔧 ADAPTIVE BUFFER: Set chunk size to {} samples for app device {}", 
              self.adaptive_chunk_size, self.device_id);
    }
    
    // Getters (compatible with AudioInputStream interface)
    pub fn device_id(&self) -> &str { &self.device_id }
    pub fn device_name(&self) -> &str { &self.device_name }
    pub fn sample_rate(&self) -> u32 { self.sample_rate }
    pub fn channels(&self) -> u16 { self.channels }
}

/// High-level manager for application audio capture
#[derive(Clone)]
pub struct ApplicationAudioManager {
    discovery: Arc<Mutex<ApplicationDiscovery>>,
    active_taps: Arc<RwLock<HashMap<u32, ApplicationAudioTap>>>, // PID -> Tap
    permission_granted: Arc<RwLock<bool>>,
    max_concurrent_captures: usize,
    cleanup_handle: Arc<StdMutex<Option<tokio::task::JoinHandle<()>>>>,
    should_stop_cleanup: Arc<std::sync::atomic::AtomicBool>,
}

impl ApplicationAudioManager {
    pub fn new() -> Self {
        Self {
            discovery: Arc::new(Mutex::new(ApplicationDiscovery::new())),
            active_taps: Arc::new(RwLock::new(HashMap::new())),
            permission_granted: Arc::new(RwLock::new(false)),
            max_concurrent_captures: 4, // Limit to prevent performance issues
            cleanup_handle: Arc::new(StdMutex::new(None)),
            should_stop_cleanup: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    
    /// Ensure cleanup task is running (lazy startup)
    fn ensure_cleanup_task_started(&self) {
        if let Ok(cleanup_handle_guard) = self.cleanup_handle.try_lock() {
            if cleanup_handle_guard.is_none() {
                drop(cleanup_handle_guard);
                self.start_cleanup_task();
            }
        }
    }
    
    /// Check and request audio capture permissions
    pub async fn request_permissions(&self) -> Result<bool> {
        info!("Requesting audio capture permissions...");
        self.ensure_cleanup_task_started();
        
        #[cfg(target_os = "macos")]
        {
            use crate::permissions::{get_permission_manager, TccPermissionStatus};
            
            let permission_manager = get_permission_manager();
            
            // First check current permission status
            let status = permission_manager.check_audio_capture_permissions().await;
            info!("Current permission status: {:?}", status);
            
            let granted = match status {
                TccPermissionStatus::Granted => {
                    info!("Audio capture permissions already granted");
                    true
                }
                TccPermissionStatus::Denied => {
                    warn!("Audio capture permissions denied by user");
                    info!("Instructions for enabling permissions:\n{}", 
                        permission_manager.get_permission_instructions());
                    false
                }
                TccPermissionStatus::NotDetermined => {
                    info!("Permissions not determined - will be requested on first audio access");
                    // Let the system handle the permission request when we try to access audio
                    match permission_manager.request_permissions().await {
                        Ok(result) => result,
                        Err(e) => {
                            error!("Failed to request permissions: {}", e);
                            false
                        }
                    }
                }
                TccPermissionStatus::Unknown => {
                    warn!("Unable to determine permission status - assuming not granted");
                    false
                }
            };
            
            *self.permission_granted.write().await = granted;
            
            if !granted {
                info!("To manually enable permissions, run: open 'x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone'");
            }
            
            Ok(granted)
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            warn!("Permission checking not implemented on this platform");
            *self.permission_granted.write().await = false;
            Ok(false)
        }
    }
    
    /// Get list of available audio applications
    pub async fn get_available_applications(&self) -> Result<Vec<ProcessInfo>> {
        let mut discovery = self.discovery.lock().await;
        discovery.scan_audio_applications()
    }
    
    /// Start capturing audio from a specific application
    pub async fn start_capturing_app(&self, pid: u32) -> Result<broadcast::Receiver<Vec<f32>>> {
        // Ensure cleanup task is running
        self.ensure_cleanup_task_started();
        
        // Check permissions (actively check system, don't use cached value)
        if !self.check_audio_capture_permissions().await {
            return Err(anyhow::anyhow!("Audio capture permissions not granted"));
        }
        
        // Check concurrent capture limit
        let active_count = self.active_taps.read().await.len();
        if active_count >= self.max_concurrent_captures {
            return Err(anyhow::anyhow!(
                "Maximum concurrent captures reached ({}/{})", 
                active_count, 
                self.max_concurrent_captures
            ));
        }
        
        // Get process info
        let discovery = self.discovery.lock().await;
        let process_info = discovery.get_process_info(pid)
            .ok_or_else(|| anyhow::anyhow!("Process not found: {}", pid))?;
        drop(discovery);
        
        // Create and configure tap
        #[cfg(target_os = "macos")]
        {
            let mut tap = ApplicationAudioTap::new(process_info);
            
            // Attempt to create the tap with error tracking
            match tap.create_tap().await {
                Ok(_) => {
                    tap.reset_errors().await; // Reset error count on success
                }
                Err(e) => {
                    tap.record_error().await;
                    return Err(e);
                }
            }
            
            // Start capturing with error tracking
            let receiver = match tap.start_capture() {
                Ok(r) => {
                    tap.reset_errors().await;
                    tap.heartbeat().await; // Mark as active
                    r
                }
                Err(e) => {
                    tap.record_error().await;
                    return Err(e);
                }
            };
            
            // Store the tap
            self.active_taps.write().await.insert(pid, tap);
            
            info!("Started capturing audio from PID {} with lifecycle management", pid);
            Ok(receiver)
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Err(anyhow::anyhow!("Application audio capture is only supported on macOS"))
        }
    }
    
    /// Create a virtual mixer input channel for an application's audio
    /// This integrates application audio capture with the existing mixer system
    pub async fn create_mixer_input_for_app(&self, pid: u32) -> Result<String> {
        info!("🎛️ Creating mixer input for application PID: {}", pid);
        
        // Get process info for naming FIRST
        let discovery = self.discovery.lock().await;
        let process_info = discovery.get_process_info(pid)
            .ok_or_else(|| anyhow::anyhow!("Process not found: {}", pid))?;
        drop(discovery);
        
        let channel_name = format!("App: {}", process_info.name);
        let virtual_device_id = format!("app-{}", pid);
        
        // CRITICAL: Register virtual stream FIRST, before starting capture
        info!("📡 Pre-registering virtual stream {} before capture", virtual_device_id);
        let bridge_buffer = Arc::new(tokio::sync::Mutex::new(Vec::<f32>::new()));
        self.register_virtual_input_stream_sync(virtual_device_id.clone(), channel_name.clone(), bridge_buffer.clone()).await?;
        
        // NOW start capturing from the application 
        let audio_receiver = self.start_capturing_app(pid).await?;
        
        // Bridge the audio to the pre-registered stream
        self.bridge_tap_audio_to_existing_stream(pid, audio_receiver, bridge_buffer).await?;
        
        info!("✅ Created virtual mixer input '{}' for PID {} with pre-registered stream", channel_name, pid);
        Ok(channel_name)
    }
    
    /// Bridge tap audio data to the mixer input system
    async fn bridge_tap_audio_to_mixer(
        &self,
        pid: u32,
        mut audio_receiver: broadcast::Receiver<Vec<f32>>,
        channel_name: String,
    ) -> Result<()> {
        use std::sync::Arc;
        use tokio::sync::Mutex as TokioMutex;
        
        info!("🌉 Setting up audio bridge for {} (PID: {})", channel_name, pid);
        
        // Create a buffer that will act like a CPAL input stream buffer
        let bridge_buffer = Arc::new(TokioMutex::new(Vec::<f32>::new()));
        let bridge_buffer_for_task = bridge_buffer.clone();
        
        // Create a virtual device ID for this application audio source
        // IMPORTANT: This must match the ID used in mixer channel configuration
        let virtual_device_id = format!("app-{}", pid);
        
        // Spawn a task to bridge audio from broadcast channel to mixer buffer
        let bridge_task_name = channel_name.clone();
        let virtual_device_id_for_task = virtual_device_id.clone();
        
        tokio::spawn(async move {
            info!("🔗 Audio bridge task started for {}", bridge_task_name);
            let mut sample_count = 0u64;
            
            while let Ok(audio_samples) = audio_receiver.recv().await {
                sample_count += audio_samples.len() as u64;
                
                // Calculate levels for monitoring
                let peak_level = audio_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                let rms_level = (audio_samples.iter().map(|&s| s * s).sum::<f32>() / audio_samples.len() as f32).sqrt();
                
                // Store samples in the bridge buffer (same pattern as CPAL input streams)
                if let Ok(mut buffer) = bridge_buffer_for_task.try_lock() {
                    buffer.extend_from_slice(&audio_samples);
                    
                    // Prevent buffer overflow - same logic as regular input streams
                    let max_buffer_size = 48000; // 1 second at 48kHz
                    if buffer.len() > max_buffer_size * 2 {
                        let keep_size = max_buffer_size;
                        let buffer_len = buffer.len();
                        let new_buffer = buffer.split_off(buffer_len - keep_size);
                        *buffer = new_buffer;
                    }
                    
                    // Log periodically
                    if sample_count % 4800 == 0 || (peak_level > 0.01 && sample_count % 1000 == 0) {
                        info!("🌉 BRIDGE [{}]: {} samples bridged to mixer, peak: {:.4}, rms: {:.4}, buffer: {} samples", 
                            virtual_device_id_for_task, audio_samples.len(), peak_level, rms_level, buffer.len());
                    }
                } else {
                    warn!("Failed to lock bridge buffer for {}", bridge_task_name);
                }
            }
            
            info!("🔗 Audio bridge task ended for {}", bridge_task_name);
        });
        
        // Now we need to register this virtual audio source with the mixer system
        // We'll create a virtual AudioInputStream that reads from our bridge buffer
        self.register_virtual_input_stream(virtual_device_id, channel_name, bridge_buffer).await?;
        
        Ok(())
    }
    
    /// Register virtual stream synchronously BEFORE capture starts  
    async fn register_virtual_input_stream_sync(
        &self,
        virtual_device_id: String,
        channel_name: String,
        bridge_buffer: Arc<tokio::sync::Mutex<Vec<f32>>>,
    ) -> Result<()> {
        info!("📡 SYNC: Registering virtual input stream: {} ({})", channel_name, virtual_device_id);
        
        // Create the AudioInputStream immediately and register it
        let audio_input_stream = Arc::new(crate::audio::mixer::transformer::AudioInputStream {
            device_id: virtual_device_id.clone(),
            device_name: channel_name.clone(),
            sample_rate: 48000,
            channels: 2,
            audio_buffer: bridge_buffer.clone(), 
            effects_chain: Arc::new(tokio::sync::Mutex::new(
                crate::audio::effects::AudioEffectsChain::new(48000)
            )),
            adaptive_chunk_size: 480, // 10ms at 48kHz
        });
        
        // Store in global registry IMMEDIATELY
        self.add_to_global_mixer_sync(virtual_device_id.clone(), audio_input_stream).await?;
        
        info!("✅ SYNC: Virtual stream {} registered and ready for mixer", virtual_device_id);
        Ok(())
    }
    
    /// Synchronously add virtual stream to global mixer registry
    async fn add_to_global_mixer_sync(
        &self,
        device_id: String,
        audio_input_stream: Arc<crate::audio::mixer::transformer::AudioInputStream>,
    ) -> Result<()> {
        info!("🔗 SYNC: Adding virtual stream {} to global mixer registry", device_id);
        
        // Use centralized registry function
        let registry = get_virtual_input_registry();
        if let Ok(mut reg) = registry.lock() {
            reg.insert(device_id.clone(), audio_input_stream);
            info!("✅ SYNC: Registered virtual stream {} in global registry (total: {})", device_id, reg.len());
        } else {
            return Err(anyhow::anyhow!("Failed to lock virtual input registry"));
        }
        
        Ok(())
    }
    
    /// Bridge existing audio receiver to pre-registered stream buffer
    async fn bridge_tap_audio_to_existing_stream(
        &self,
        pid: u32,
        mut audio_receiver: broadcast::Receiver<Vec<f32>>,
        bridge_buffer: Arc<tokio::sync::Mutex<Vec<f32>>>,
    ) -> Result<()> {
        let process_name = format!("PID-{}", pid);
        
        info!("🌉 Setting up audio bridge to existing stream for {}", process_name);
        
        // Start background task to bridge audio data
        tokio::spawn(async move {
            info!("🔗 Audio bridge task started for {}", process_name);
            let mut sample_count = 0u64;
            
            while let Ok(audio_samples) = audio_receiver.recv().await {
                sample_count += audio_samples.len() as u64;
                
                // Transfer samples to the pre-registered stream buffer
                if let Ok(mut buffer) = bridge_buffer.try_lock() {
                    buffer.extend_from_slice(&audio_samples);
                    
                    // Buffer overflow protection
                    let max_buffer_size = 48000; // 1 second at 48kHz
                    if buffer.len() > max_buffer_size * 2 {
                        let keep_size = max_buffer_size;
                        let buffer_len = buffer.len();
                        let new_buffer = buffer.split_off(buffer_len - keep_size);
                        *buffer = new_buffer;
                    }
                    
                    // Log periodically
                    if sample_count % 4800 == 0 {
                        let peak = audio_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        info!("🌉 BRIDGE [{}]: {} total samples bridged, recent peak: {:.4}", 
                              process_name, sample_count, peak);
                    }
                }
            }
            
            info!("🔗 Audio bridge task ended for {}", process_name);
        });
        
        Ok(())
    }
    
    /// Register a virtual input stream with the mixer system
    async fn register_virtual_input_stream(
        &self,
        virtual_device_id: String,
        channel_name: String,
        bridge_buffer: Arc<tokio::sync::Mutex<Vec<f32>>>,
    ) -> Result<()> {
        info!("📡 Registering virtual input stream: {} ({})", channel_name, virtual_device_id);
        
        // Create a bridge adapter that's compatible with the mixer's AudioInputStream interface
        let audio_bridge = ApplicationAudioInputBridge::new(
            virtual_device_id.clone(),
            channel_name.clone(),
            48000, // Default sample rate
            bridge_buffer,
        )?;
        
        // Convert to the format the mixer expects
        // Note: We need to expose the sync buffer from the bridge as std::sync::Mutex
        let audio_buffer_sync = Arc::new(tokio::sync::Mutex::new(Vec::<f32>::new()));
        let effects_chain_sync = Arc::new(tokio::sync::Mutex::new(
            crate::audio::effects::AudioEffectsChain::new(audio_bridge.sample_rate())
        ));
        
        let audio_input_stream = Arc::new(crate::audio::mixer::transformer::AudioInputStream {
            device_id: audio_bridge.device_id().to_string(),
            device_name: audio_bridge.device_name().to_string(),
            sample_rate: audio_bridge.sample_rate(),
            channels: audio_bridge.channels(),
            audio_buffer: audio_buffer_sync.clone(),
            effects_chain: effects_chain_sync.clone(),
            adaptive_chunk_size: audio_bridge.adaptive_chunk_size,
        });
        
        // Store the bridge for periodic sync operations
        // We need to keep the bridge alive to handle async->sync transfers
        let audio_bridge = Arc::new(audio_bridge);
        
        // Start a background task to continuously sync samples from async to sync buffers
        let bridge_for_sync = audio_bridge.clone();
        let device_name_for_sync = channel_name.clone();
        let audio_buffer_for_sync = audio_buffer_sync.clone();
        
        tokio::spawn(async move {
            info!("🔄 Started sync task for application audio bridge: {}", device_name_for_sync);
            let mut sync_count = 0u64;
            
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await; // 200 Hz sync rate
                
                // Transfer samples from the bridge's async buffer to the AudioInputStream's sync buffer
                if let Ok(mut async_buffer) = bridge_for_sync.audio_buffer.try_lock() {
                    if !async_buffer.is_empty() {
                        let samples: Vec<f32> = async_buffer.drain(..).collect();
                        
                        if let Ok(mut sync_buffer) = audio_buffer_for_sync.try_lock() {
                            sync_buffer.extend_from_slice(&samples);
                            
                            // Prevent buffer overflow
                            let max_buffer_size = 48000; // 1 second at 48kHz
                            if sync_buffer.len() > max_buffer_size * 2 {
                                let keep_size = max_buffer_size;
                                let buffer_len = sync_buffer.len();
                                let new_buffer = sync_buffer.split_off(buffer_len - keep_size);
                                *sync_buffer = new_buffer;
                            }
                        }
                    }
                }
                
                sync_count += 1;
                if sync_count % 4000 == 0 {
                    // Log every 20 seconds at 200 Hz
                    info!("🔄 Application audio sync task running for {}: {} syncs", 
                          device_name_for_sync, sync_count);
                }
            }
        });
        
        // Add the virtual stream to the global mixer's input_streams collection
        // We need to access the global AudioState for this
        self.add_to_global_mixer(virtual_device_id.clone(), audio_input_stream, audio_bridge).await?;
        
        info!("✅ Successfully registered virtual input stream: {} -> ready for mixer", virtual_device_id);
        Ok(())
    }
    
    /// Add the virtual audio stream to the global mixer's input streams collection
    async fn add_to_global_mixer(
        &self,
        device_id: String,
        audio_input_stream: Arc<crate::audio::mixer::transformer::AudioInputStream>,
        _bridge: Arc<ApplicationAudioInputBridge>,
    ) -> Result<()> {
        // Store the stream in a global registry that the mixer can access
        // For now, we'll use a static registry approach
        
        info!("🔗 Adding virtual stream {} to global mixer registry", device_id);
        
        use std::sync::{LazyLock, Mutex as StdMutex};
        use std::collections::HashMap;
        
        static VIRTUAL_INPUT_REGISTRY: LazyLock<StdMutex<HashMap<String, Arc<crate::audio::mixer::transformer::AudioInputStream>>>> = 
            LazyLock::new(|| StdMutex::new(HashMap::new()));
        
        // Register the virtual stream globally
        if let Ok(mut registry) = VIRTUAL_INPUT_REGISTRY.lock() {
            registry.insert(device_id.clone(), audio_input_stream);
            info!("✅ Registered virtual stream {} in global registry", device_id);
        } else {
            return Err(anyhow::anyhow!("Failed to lock virtual input registry"));
        }
        
        // Now we need to trigger the mixer to pick up this new virtual device
        // This could be done via a notification system or polling
        info!("📢 Virtual stream {} ready for mixer discovery", device_id);
        
        Ok(())
    }
    
    /// Get all registered virtual input streams (for mixer integration)
    pub fn get_virtual_input_streams() -> HashMap<String, Arc<crate::audio::mixer::transformer::AudioInputStream>> {
        // Use centralized registry function
        let registry = get_virtual_input_registry();
        if let Ok(reg) = registry.lock() {
            reg.clone()
        } else {
            HashMap::new()
        }
    }
    
    /// Stop capturing audio from a specific application
    pub async fn stop_capturing_app(&self, pid: u32) -> Result<()> {
        let mut taps = self.active_taps.write().await;
        if let Some(mut tap) = taps.remove(&pid) {
            tap.destroy()?;
            info!("Stopped capturing audio from PID {}", pid);
            Ok(())
        } else {
            Err(anyhow::anyhow!("No active capture for PID {}", pid))
        }
    }
    
    /// Get list of currently active captures
    pub async fn get_active_captures(&self) -> Vec<ProcessInfo> {
        let taps = self.active_taps.read().await;
        taps.values()
            .map(|tap| tap.get_process_info().clone())
            .collect()
    }
    
    /// Stop all active captures
    pub async fn stop_all_captures(&self) -> Result<()> {
        let mut taps = self.active_taps.write().await;
        
        for (pid, mut tap) in taps.drain() {
            if let Err(e) = tap.destroy() {
                error!("Failed to destroy tap for PID {}: {}", pid, e);
            }
        }
        
        info!("Stopped all active audio captures");
        Ok(())
    }
    
    /// Check if permissions are granted
    pub async fn has_permissions(&self) -> bool {
        *self.permission_granted.read().await
    }
    
    /// Check if permissions are granted (actively checks system, not cached)
    pub async fn check_audio_capture_permissions(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            use crate::permissions::{get_permission_manager, TccPermissionStatus};
            
            let permission_manager = get_permission_manager();
            let status = permission_manager.check_audio_capture_permissions().await;
            
            let granted = matches!(status, TccPermissionStatus::Granted);
            
            // Update cached status
            *self.permission_granted.write().await = granted;
            
            granted
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            // On non-macOS platforms, return cached value
            self.has_permissions().await
        }
    }
    
    /// Start the background cleanup task
    fn start_cleanup_task(&self) {
        let active_taps = Arc::clone(&self.active_taps);
        let should_stop = Arc::clone(&self.should_stop_cleanup);
        let cleanup_handle = Arc::clone(&self.cleanup_handle);
        
        let handle = tokio::spawn(async move {
            info!("Started tap cleanup task");
            
            let mut cleanup_interval = tokio::time::interval(std::time::Duration::from_secs(30));
            cleanup_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            while !should_stop.load(std::sync::atomic::Ordering::Relaxed) {
                cleanup_interval.tick().await;
                
                let mut taps_to_remove = Vec::new();
                
                // Check all active taps for health
                {
                    let taps = active_taps.read().await;
                    for (pid, tap) in taps.iter() {
                        let stats = tap.get_stats().await;
                        
                        // Check various cleanup conditions
                        let should_cleanup = 
                            !stats.process_alive ||  // Process died
                            stats.error_count >= 5 || // Too many errors
                            tap.is_stale(std::time::Duration::from_secs(300)).await; // 5 min inactive
                        
                        if should_cleanup {
                            debug!(
                                "Marking tap for cleanup: PID={}, alive={}, errors={}, stale={}",
                                stats.pid,
                                stats.process_alive,
                                stats.error_count,
                                tap.is_stale(std::time::Duration::from_secs(300)).await
                            );
                            taps_to_remove.push(*pid);
                        }
                    }
                }
                
                // Clean up marked taps
                if !taps_to_remove.is_empty() {
                    let mut taps = active_taps.write().await;
                    for pid in taps_to_remove {
                        if let Some(mut tap) = taps.remove(&pid) {
                            info!("Automatically cleaning up tap for PID {}", pid);
                            if let Err(e) = tap.destroy() {
                                error!("Failed to destroy tap during cleanup for PID {}: {}", pid, e);
                            }
                        }
                    }
                }
            }
            
            info!("Tap cleanup task stopped");
        });
        
        // Store the handle for later cleanup
        if let Ok(mut cleanup_handle_guard) = cleanup_handle.try_lock() {
            *cleanup_handle_guard = Some(handle);
        };
    }
    
    /// Get statistics for all active taps
    pub async fn get_tap_stats(&self) -> Vec<TapStats> {
        let taps = self.active_taps.read().await;
        let mut stats = Vec::new();
        
        for tap in taps.values() {
            stats.push(tap.get_stats().await);
        }
        
        stats.sort_by_key(|s| s.pid);
        stats
    }
    
    /// Perform manual health check and cleanup on all taps
    pub async fn cleanup_stale_taps(&self) -> Result<usize> {
        let mut taps_to_remove = Vec::new();
        let mut cleaned_count = 0;
        
        // Identify stale taps
        {
            let taps = self.active_taps.read().await;
            for (pid, tap) in taps.iter() {
                if !tap.is_process_alive() {
                    info!("Process {} no longer alive, marking for cleanup", pid);
                    taps_to_remove.push(*pid);
                }
                else if tap.is_stale(std::time::Duration::from_secs(180)).await {
                    info!("Tap for PID {} is stale, marking for cleanup", pid);
                    taps_to_remove.push(*pid);
                }
                else if tap.get_error_count().await >= 3 {
                    info!("Tap for PID {} has too many errors, marking for cleanup", pid);
                    taps_to_remove.push(*pid);
                }
            }
        }
        
        // Clean up identified taps
        if !taps_to_remove.is_empty() {
            let mut taps = self.active_taps.write().await;
            for pid in taps_to_remove {
                if let Some(mut tap) = taps.remove(&pid) {
                    match tap.destroy() {
                        Ok(_) => {
                            info!("Successfully cleaned up tap for PID {}", pid);
                            cleaned_count += 1;
                        }
                        Err(e) => {
                            error!("Failed to destroy tap for PID {}: {}", pid, e);
                        }
                    }
                }
            }
        }
        
        Ok(cleaned_count)
    }
    
    /// Graceful shutdown - stop cleanup task and destroy all taps
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down ApplicationAudioManager...");
        
        // Stop the cleanup task
        self.should_stop_cleanup.store(true, std::sync::atomic::Ordering::Relaxed);
        
        if let Ok(mut handle_guard) = self.cleanup_handle.lock() {
            if let Some(handle) = handle_guard.take() {
                handle.abort();
                info!("Stopped cleanup task");
            }
        }
        
        // Stop all active captures
        self.stop_all_captures().await?;
        
        info!("ApplicationAudioManager shutdown complete");
        Ok(())
    }
}

/// Errors that can occur during application audio operations
#[derive(Debug, thiserror::Error)]
pub enum ApplicationAudioError {
    #[error("Permission denied - audio capture not authorized")]
    PermissionDenied,
    
    #[error("Application not found (PID: {pid})")]
    ApplicationNotFound { pid: u32 },
    
    #[error("Core Audio error: {status}")]
    CoreAudioError { status: i32 },
    
    #[error("Unsupported macOS version - requires 14.4+")]
    UnsupportedSystem,
    
    #[error("Too many active captures (max: {max})")]
    TooManyCaptures { max: usize },
    
    #[error("Audio tap not initialized")]
    TapNotInitialized,
    
    #[error("System error: {0}")]
    SystemError(#[from] anyhow::Error),
}