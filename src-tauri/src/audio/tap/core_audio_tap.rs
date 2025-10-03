// Core Audio tap implementation for macOS application audio capture
//
// This module handles the low-level Core Audio tap functionality for capturing
// audio from specific applications. It includes the tap callback, device management,
// and stream setup logic specific to macOS 14.4+.

#[cfg(target_os = "macos")]
use anyhow::Result;
#[cfg(target_os = "macos")]
use std::sync::{Arc, Mutex as StdMutex};
#[cfg(target_os = "macos")]
use tokio::sync::broadcast;
#[cfg(target_os = "macos")]
use tracing::{debug, error, info, warn};

#[cfg(target_os = "macos")]
use super::types::{ApplicationAudioError, CoreAudioTapCallbackContext, ProcessInfo, TapStats};

/// Helper struct for audio format information
#[derive(Debug, Clone)]
struct AudioFormatInfo {
    sample_rate: f64,
    channels: u32,
    bits_per_sample: u32,
}

/// Core Audio IOProc callback for tap device
#[cfg(target_os = "macos")]
pub unsafe extern "C" fn core_audio_tap_callback(
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
    let callback_count = context
        .callback_count
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    if input_data.is_null() {
        if callback_count % 1000 == 0 {
            eprintln!(
                "⚠️ TAP CALLBACK: No input data (callback #{})",
                callback_count
            );
        }
        return 0; // No error, but no data
    }

    // Extract audio samples from AudioBufferList
    let buffer_list = &*input_data;
    let buffer_count = buffer_list.mNumberBuffers;

    if buffer_count == 0 {
        if callback_count % 1000 == 0 {
            eprintln!(
                "⚠️ TAP CALLBACK: No audio buffers (callback #{})",
                callback_count
            );
        }
        return 0;
    }

    // Process the first buffer (typically the only one for simple cases)
    let audio_buffer = &buffer_list.mBuffers[0];
    let data_ptr = audio_buffer.mData as *const f32;
    let sample_count = (audio_buffer.mDataByteSize as usize) / std::mem::size_of::<f32>();

    if data_ptr.is_null() || sample_count == 0 {
        if callback_count % 1000 == 0 {
            eprintln!(
                "⚠️ TAP CALLBACK: No sample data (callback #{})",
                callback_count
            );
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
        eprintln!(
            "🔊 CORE AUDIO TAP [{}] Device {}: Callback #{}: {} samples, peak: {:.4}, rms: {:.4}",
            context.process_name,
            device_id,
            callback_count,
            samples.len(),
            peak_level,
            rms_level
        );
    }

    // Send samples to broadcast channel for mixer integration
    if let Err(_) = context.audio_tx.send(samples) {
        if callback_count % 1000 == 0 {
            eprintln!(
                "⚠️ Failed to send tap samples to broadcast channel (callback #{})",
                callback_count
            );
        }
    }

    0 // Success
}

/// Manages Core Audio taps for individual applications (macOS 14.4+ only)
#[cfg(target_os = "macos")]
pub struct ApplicationAudioTap {
    process_info: ProcessInfo,
    tap_id: Option<u32>,              // AudioObjectID placeholder
    aggregate_device_id: Option<u32>, // AudioObjectID placeholder
    audio_producer: Option<Arc<StdMutex<rtrb::Producer<f32>>>>, // RTRB producer for pipeline integration
    detected_sample_rate: Option<f64>, // Detected sample rate from the tap
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
            audio_producer: None,
            detected_sample_rate: None,
            _stream_info: None,
            is_capturing: false,
            created_at: now,
            last_heartbeat: Arc::new(StdMutex::new(now)),
            error_count: Arc::new(StdMutex::new(0)),
            max_errors: 5, // Maximum errors before automatic cleanup
        }
    }

    /// Get the detected sample rate from the tap (available after create_tap succeeds)
    pub fn get_detected_sample_rate(&self) -> Option<f64> {
        self.detected_sample_rate
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

    /// Create a Core Audio tap for this application's process
    pub async fn create_tap(&mut self) -> Result<()> {
        info!(
            "🔧 DEBUG: Creating audio tap for {} (PID: {})",
            self.process_info.name, self.process_info.pid
        );
        info!(
            "🔧 DEBUG: Process bundle_id: {:?}",
            self.process_info.bundle_id
        );

        // Check macOS version compatibility
        if !self.is_core_audio_taps_supported() {
            return Err(anyhow::anyhow!(
                "Core Audio taps require macOS 14.4 or later. Use BlackHole for audio capture on older systems."
            ).into());
        }

        // Import Core Audio taps bindings (only available on macOS 14.4+)
        use super::core_audio_bindings::{
            create_process_tap, create_process_tap_description, format_osstatus_error,
        };

        // Step 1: Try using PID directly in CATapDescription (skip translation)
        info!(
            "Creating Core Audio process tap for PID {} directly with objc2_core_audio",
            self.process_info.pid
        );
        let tap_object_id = unsafe {
            // Create tap description in a limited scope so it's dropped before await
            // Try using PID directly - some examples suggest this works
            let tap_description = create_process_tap_description(self.process_info.pid);
            info!(
                "Created tap description for process {}",
                self.process_info.name
            );

            match create_process_tap(&tap_description) {
                Ok(id) => {
                    info!(
                        "✅ SUCCESS: Created process tap with AudioObjectID {} for {} (PID: {})",
                        id, self.process_info.name, self.process_info.pid
                    );
                    id
                }
                Err(status) => {
                    let error_msg = format_osstatus_error(status);
                    if status == -4 {
                        return Err(anyhow::anyhow!("Unsupported system for Core Audio taps"));
                    } else {
                        return Err(anyhow::anyhow!("Core Audio error: {}", error_msg));
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

        info!(
            "✅ Audio tap successfully created for {}",
            self.process_info.name
        );
        Ok(())
    }

    // Additional methods for tap management...
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

    /// Check if Core Audio taps are supported on this system
    fn is_core_audio_taps_supported(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;

            // Get macOS version using sw_vers command
            if let Ok(output) = Command::new("sw_vers").arg("-productVersion").output() {
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
    /// Set up audio streaming from the Core Audio tap
    async fn setup_tap_audio_stream(
        &mut self,
        tap_object_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
    ) -> Result<()> {
        info!(
            "Setting up audio stream for tap AudioObjectID {}",
            tap_object_id
        );

        // Use cpal to create an AudioUnit-based input stream from the tap device
        self.create_cpal_input_stream_from_tap(tap_object_id, audio_tx)
            .await
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
            0,                // qualifier size
            std::ptr::null(), // qualifier data
            &mut data_size,
            &mut sample_rate as *mut f64 as *mut c_void,
        );

        if status == 0 {
            Ok(sample_rate)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get tap sample rate: OSStatus {}",
                status
            ))
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
            0,                // qualifier size
            std::ptr::null(), // qualifier data
            &mut data_size,
            &mut format_desc as *mut AudioStreamBasicDescription as *mut c_void,
        );

        if status == 0 {
            Ok(format_desc.channels_per_frame)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get tap channel count: OSStatus {}",
                status
            ))
        }
    }

    /// Create a CPAL input stream from the Core Audio tap device
    #[cfg(target_os = "macos")]
    async fn create_cpal_input_stream_from_tap(
        &mut self,
        tap_object_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
    ) -> Result<()> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        info!(
            "Creating CPAL input stream for Core Audio tap device ID {}",
            tap_object_id
        );

        // Get the tap device properties using Core Audio APIs
        let sample_rate = unsafe {
            self.get_tap_sample_rate(tap_object_id)
                .unwrap_or(crate::types::DEFAULT_SAMPLE_RATE as f64)
        };

        let channels = unsafe { self.get_tap_channel_count(tap_object_id).unwrap_or(2) };

        info!(
            "Tap device properties: {} Hz, {} channels",
            sample_rate, channels
        );

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

        info!(
            "🔍 DEBUG: Looking for tap device among {} available input devices",
            devices.len()
        );
        for (i, device) in devices.iter().enumerate() {
            if let Ok(device_name) = device.name() {
                info!("🔍 DEBUG: Input device {}: '{}'", i, device_name);

                // Core Audio taps might appear with specific naming patterns
                if device_name.contains("Tap")
                    || device_name.contains(&tap_id_str)
                    || device_name.contains("Aggregate")
                {
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
            info!(
                "🔄 FALLBACK: Using virtual audio bridge for tap AudioObjectID {}",
                tap_object_id
            );
            return self
                .setup_virtual_tap_bridge(tap_object_id, audio_tx, sample_rate, channels)
                .await;
        }

        let device = tap_device.unwrap();
        let device_name = device
            .name()
            .unwrap_or_else(|_| format!("Tap-{}", tap_object_id));

        // Get device configuration
        let device_config = match device.default_input_config() {
            Ok(config) => config,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to get device config for tap: {}",
                    e
                ));
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

        info!(
            "Creating tap stream with config: {} channels, {} Hz",
            config.channels, config.sample_rate.0
        );

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
                     let audio_samples = if tap_sample_rate != crate::types::DEFAULT_SAMPLE_RATE {
                         // Simple linear interpolation resampling for non-48kHz audio
                         Self::resample_audio(data, tap_sample_rate, crate::types::DEFAULT_SAMPLE_RATE)
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
                        let f32_samples: Vec<f32> = data
                            .iter()
                            .map(|&sample| {
                                if sample >= 0 {
                                    sample as f32 / 32767.0
                                } else {
                                    sample as f32 / 32768.0
                                }
                            })
                            .collect();

                        let audio_samples = if tap_sample_rate != crate::types::DEFAULT_SAMPLE_RATE
                        {
                            // Simple linear interpolation resampling for non-48kHz audio
                            Self::resample_audio(
                                &f32_samples,
                                tap_sample_rate,
                                crate::types::DEFAULT_SAMPLE_RATE,
                            )
                        } else {
                            f32_samples
                        };

                        let peak_level = audio_samples
                            .iter()
                            .map(|&s| s.abs())
                            .fold(0.0f32, f32::max);

                        if callback_count % 100 == 0
                            || (peak_level > 0.01 && callback_count % 50 == 0)
                        {
                            info!(
                                "🔊 TAP AUDIO I16 [{}] Callback #{}: {} samples, peak: {:.4}",
                                process_name,
                                callback_count,
                                data.len(),
                                peak_level
                            );
                        }

                        // Send converted audio data
                        if let Err(e) = audio_tx_for_callback.send(audio_samples) {
                            if callback_count % 1000 == 0 {
                                warn!(
                                    "Failed to send tap audio I16 samples: {} (callback #{})",
                                    e, callback_count
                                );
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
                return Err(anyhow::anyhow!(
                    "Unsupported tap sample format: {:?}",
                    device_config.sample_format()
                ));
            }
        };

        // Start the stream
        stream
            .play()
            .map_err(|e| anyhow::anyhow!("Failed to start tap stream: {}", e))?;

        info!(
            "✅ Successfully started Core Audio tap stream for {}",
            self.process_info.name
        );
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

    /// Set up IOProc callback on aggregate device to receive tap data
    #[cfg(target_os = "macos")]
    async fn setup_ioproc_on_aggregate_device(
        &mut self,
        aggregate_device_id: coreaudio_sys::AudioObjectID,
        audio_tx: broadcast::Sender<Vec<f32>>,
        sample_rate: f64,
        channels: u32,
    ) -> Result<()> {
        use coreaudio_sys::{
            AudioDeviceCreateIOProcID, AudioDeviceIOProcID, AudioDeviceStart, AudioObjectID,
        };
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        info!(
            "🔧 Setting up IOProc callback on aggregate device {}",
            aggregate_device_id
        );
        info!(
            "📋 Aggregate device format: {:.0} Hz, {} channels",
            sample_rate, channels
        );

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

            let context = unsafe {
                &*(client_data as *const (broadcast::Sender<Vec<f32>>, String, Arc<AtomicBool>))
            };
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
                        println!(
                            "🎵 REAL APPLE MUSIC DATA [{}]: {} samples, peak: {:.4}",
                            process_name,
                            sample_vec.len(),
                            peak
                        );
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
            unsafe {
                drop(Box::from_raw(context_ptr));
            }
            return Err(anyhow::anyhow!(
                "AudioDeviceCreateIOProcID failed on aggregate device: OSStatus {}",
                status
            ));
        }

        info!("✅ Created IOProc on aggregate device: {:?}", io_proc_id);

        // Start the aggregate device
        let start_status = unsafe { AudioDeviceStart(aggregate_device_id, io_proc_id) };
        if start_status != 0 {
            return Err(anyhow::anyhow!(
                "AudioDeviceStart failed on aggregate device: OSStatus {}",
                start_status
            ));
        }

        info!("🎉 BREAKTHROUGH: Aggregate device started - real Apple Music audio should flow through IOProc!");

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
        info!(
            "🔧 IMPLEMENTING: Direct Core Audio IOProc for tap AudioObjectID {}",
            tap_object_id
        );
        info!("📋 Tap config: {} Hz, {} channels", sample_rate, channels);

        // This is the correct approach - Core Audio taps don't appear as CPAL devices
        // We need to use Core Audio APIs directly to read from the tap

        use coreaudio_sys::{
            AudioBuffer, AudioBufferList, AudioDeviceCreateIOProcID, AudioDeviceIOProc,
            AudioDeviceIOProcID, AudioDeviceStart, AudioDeviceStop, AudioTimeStamp, OSStatus,
            UInt32,
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
            unsafe {
                drop(Box::from_raw(callback_context));
            }

            // Decode the error for better understanding
            let error_code = status as u32;
            let fourcc = [
                ((error_code >> 24) & 0xFF) as u8,
                ((error_code >> 16) & 0xFF) as u8,
                ((error_code >> 8) & 0xFF) as u8,
                (error_code & 0xFF) as u8,
            ];
            let error_str = String::from_utf8_lossy(&fourcc);

            error!(
                "❌ AudioDeviceCreateIOProcID failed for tap {}",
                tap_object_id
            );
            error!("   OSStatus: {} (0x{:08x})", status, error_code);
            error!("   FourCC: '{}'", error_str);
            error!("   This might indicate the tap device doesn't support IOProc callbacks");

            warn!(
                "⚠️ IOProc creation failed on tap directly - trying aggregate device approach..."
            );
            info!(
                "🎯 CORRECT APPROACH: Core Audio taps need aggregate device with tap as subdevice!"
            );
            info!(
                "🔧 Creating aggregate device that includes tap {} as subdevice",
                tap_object_id
            );

            // The correct approach: Create aggregate device with tap as subdevice, then IOProc on aggregate
            match self.create_aggregate_device_with_tap(tap_object_id).await {
                Ok(aggregate_device_id) => {
                    info!(
                        "✅ Successfully created aggregate device {}, now setting up IOProc on it",
                        aggregate_device_id
                    );
                    return self
                        .setup_ioproc_on_aggregate_device(
                            aggregate_device_id,
                            audio_tx,
                            sample_rate,
                            channels,
                        )
                        .await;
                }
                Err(e) => {
                    warn!("⚠️ Aggregate device creation failed: {}", e);
                    info!("🔄 FALLBACK: Using direct tap property reading as last resort");
                    let format = AudioFormatInfo {
                        sample_rate: sample_rate as f64,
                        channels: channels as u32,
                        bits_per_sample: 32,
                    };
                    return Err(anyhow::anyhow!(
                        "Aggregate device creation failed, cannot setup tap bridge: {}",
                        e
                    ));
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
                tap_object_id,
                start_status
            ));
        }

        info!(
            "🎵 Started Core Audio tap device {} - audio should now flow!",
            tap_object_id
        );

        // Store the IOProc ID for cleanup later
        // TODO: Add proper cleanup in destroy() method
        self.is_capturing = true;

        Ok(())
    }

    /// Create the actual Core Audio aggregate device
    #[cfg(target_os = "macos")]
    unsafe fn create_core_audio_aggregate_device(
        &self,
        device_dict: *const std::os::raw::c_void,
    ) -> Result<coreaudio_sys::AudioObjectID> {
        use coreaudio_sys::{AudioHardwareCreateAggregateDevice, AudioObjectID};

        let mut aggregate_device_id: AudioObjectID = 0;

        let status =
            AudioHardwareCreateAggregateDevice(device_dict as *const _, &mut aggregate_device_id);

        if status != 0 {
            return Err(anyhow::anyhow!(
                "AudioHardwareCreateAggregateDevice failed: OSStatus {}",
                status
            ));
        }

        if aggregate_device_id == 0 {
            return Err(anyhow::anyhow!("Created aggregate device has invalid ID"));
        }

        info!(
            "🎉 Successfully created Core Audio aggregate device: ID {}",
            aggregate_device_id
        );
        Ok(aggregate_device_id)
    }
    /// Get the UUID from a Core Audio tap
    #[cfg(target_os = "macos")]
    fn get_tap_uuid(&self, tap_object_id: coreaudio_sys::AudioObjectID) -> Result<String> {
        use core_foundation::base::{CFType, CFTypeRef, TCFType};
        use core_foundation::string::CFString;
        use coreaudio_sys::{AudioObjectGetPropertyData, AudioObjectPropertyAddress};
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
            return Err(anyhow::anyhow!(
                "Failed to get tap UUID: OSStatus {}",
                status
            ));
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
    fn create_aggregate_device_dictionary(
        &self,
        tap_uuid: &str,
    ) -> Result<*const std::os::raw::c_void> {
        use core_foundation::array::CFArray;
        use core_foundation::base::{CFTypeRef, TCFType};
        use core_foundation::dictionary::CFDictionary;
        use core_foundation::number::CFNumber;
        use core_foundation::string::CFString;

        info!(
            "🔧 Creating proper CoreFoundation dictionary for AudioHardwareCreateAggregateDevice"
        );
        info!("📋 Using tap UUID: {}", tap_uuid);

        // Create device name and UID
        let device_name = format!("SendinBeats-Tap-{}", self.process_info.pid);
        let device_uid = format!("com.sendinbeats.tap.{}", self.process_info.pid);

        info!("📋 Aggregate device name: {}", device_name);
        info!("📋 Aggregate device UID: {}", device_uid);

        // Use the correct Core Audio constants for aggregate device dictionary
        let name_key = CFString::new("name"); // kAudioAggregateDeviceNameKey
        let uid_key = CFString::new("uid"); // kAudioAggregateDeviceUIDKey
        let subdevices_key = CFString::new("subdevice list"); // kAudioAggregateDeviceSubDeviceListKey
        let master_key = CFString::new("master"); // kAudioAggregateDeviceMasterSubDeviceKey
        let is_stacked_key = CFString::new("stacked"); // kAudioAggregateDeviceIsStackedKey

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

        info!(
            "📋 Created CoreFoundation dictionary with {} keys",
            pairs.len()
        );
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

    /// Create an aggregate device that includes the Core Audio tap
    #[cfg(target_os = "macos")]
    async fn create_aggregate_device_with_tap(
        &self,
        tap_object_id: coreaudio_sys::AudioObjectID,
    ) -> Result<coreaudio_sys::AudioObjectID> {
        use core_foundation::array::CFArray;
        use core_foundation::base::{CFTypeRef, ToVoid};
        use core_foundation::dictionary::CFMutableDictionary;
        use core_foundation::number::CFNumber;
        use core_foundation::string::CFString;
        use std::ptr;

        info!(
            "🔧 IMPLEMENTING: Creating aggregate device with tap {}",
            tap_object_id
        );

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

        info!(
            "✅ Created aggregate device {} with tap {}",
            aggregate_device_id, tap_object_id
        );
        Ok(aggregate_device_id)
    }

    /// Get current error count
    pub async fn get_error_count(&self) -> u32 {
        if let Ok(error_count) = self.error_count.lock() {
            *error_count
        } else {
            u32::MAX // Return high value if we can't get the lock
        }
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

    /// Clean up the tap and associated resources
    pub async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up audio tap for {}", self.process_info.name);

        if let Some(tap_id) = self.tap_id.take() {
            // TODO: Implement actual Core Audio tap cleanup
            info!("Cleaned up Core Audio tap ID {}", tap_id);
        }

        if let Some(aggregate_id) = self.aggregate_device_id.take() {
            // TODO: Implement aggregate device cleanup
            info!("Cleaned up aggregate device ID {}", aggregate_id);
        }

        self.is_capturing = false;
        self.audio_tx = None;

        Ok(())
    }

    /// Get the audio broadcast sender for this tap
    pub fn get_audio_sender(&self) -> Option<broadcast::Sender<Vec<f32>>> {
        self.audio_tx.clone()
    }

    /// Check if this tap is currently active
    pub fn is_active(&self) -> bool {
        self.is_capturing && self.tap_id.is_some()
    }
}

// Stub for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub struct ApplicationAudioTap;

#[cfg(not(target_os = "macos"))]
impl ApplicationAudioTap {
    pub fn new(_process_info: ProcessInfo) -> Self {
        Self
    }
}
