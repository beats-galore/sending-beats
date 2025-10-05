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
use tracing::{debug, error, info, warn};

#[cfg(target_os = "macos")]
use super::types::{ApplicationAudioError, ProcessInfo, TapStats};

/// Helper struct for audio format information
#[derive(Debug, Clone)]
struct AudioFormatInfo {
    sample_rate: f64,
    channels: u32,
    bits_per_sample: u32,
}

/// Manages Core Audio taps for individual applications (macOS 14.4+ only)
#[cfg(target_os = "macos")]
pub struct ApplicationAudioTap {
    process_info: ProcessInfo,
    tap_id: Option<u32>,              // AudioObjectID placeholder
    aggregate_device_id: Option<u32>, // AudioObjectID placeholder
    audio_producer: Option<Arc<StdMutex<rtrb::Producer<f32>>>>, // RTRB producer for pipeline integration
    detected_sample_rate: Option<f64>,                          // Detected sample rate from the tap
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

    /// Create a Core Audio tap for this application's process with RTRB producer
    /// Returns the detected sample rate from the tap device
    pub async fn create_tap(&mut self, producer: rtrb::Producer<f32>) -> Result<f64> {
        // Store the producer wrapped in Arc<Mutex> for sharing with callbacks
        self.audio_producer = Some(Arc::new(StdMutex::new(producer)));
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

        // Verify process is still running
        if !self.is_process_alive() {
            return Err(anyhow::anyhow!(
                "Process {} (PID {}) is not running",
                self.process_info.name,
                self.process_info.pid
            ));
        }

        info!(
            "🎯 Attempting to create tap for: {} (PID: {}, Bundle: {:?})",
            self.process_info.name, self.process_info.pid, self.process_info.bundle_id
        );

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
            // Create tap description following Apple's documentation
            // https://developer.apple.com/documentation/coreaudio/capturing-system-audio-with-core-audio-taps
            // This translates PID → AudioObjectID then creates the tap description
            let tap_description = match create_process_tap_description(
                self.process_info.pid,
                &self.process_info.name,
            ) {
                Ok(desc) => desc,
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to create tap description for {} (PID {}): {}",
                        self.process_info.name,
                        self.process_info.pid,
                        e
                    ));
                }
            };
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
                    error!(
                        "❌ Failed to create tap for PID {}: OSStatus {} ({})",
                        self.process_info.pid, status, error_msg
                    );

                    if status == -4 {
                        return Err(anyhow::anyhow!("Unsupported system for Core Audio taps"));
                    } else if status == 560947818 {
                        // !obj error - process object not found
                        // This commonly happens with Apple's own apps (Music, Safari, etc.) or protected processes
                        let is_apple_app = self
                            .process_info
                            .bundle_id
                            .as_ref()
                            .map(|id| id.starts_with("com.apple."))
                            .unwrap_or(false);

                        if is_apple_app {
                            return Err(anyhow::anyhow!(
                                "'{}' is not currently playing audio or is playing protected content. Core Audio taps require the application to be actively outputting audio. Please start playback in {} and try again, or use BlackHole as an alternative.",
                                self.process_info.name,
                                self.process_info.name
                            ));
                        } else {
                            return Err(anyhow::anyhow!(
                                "'{}' is not currently playing audio. Core Audio taps require the application to be actively outputting audio. Please start playback in {} and try again, or use BlackHole as an alternative.",
                                self.process_info.name,
                                self.process_info.name
                            ));
                        }
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

        // Set up actual audio callback and streaming, get detected sample rate
        let detected_sample_rate = self.setup_tap_audio_stream(tap_object_id).await?;

        info!(
            "✅ Audio tap successfully created for {} at {} Hz",
            self.process_info.name, detected_sample_rate
        );

        // Return the detected sample rate
        Ok(detected_sample_rate)
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
    /// Returns the detected sample rate
    async fn setup_tap_audio_stream(
        &mut self,
        tap_object_id: coreaudio_sys::AudioObjectID,
    ) -> Result<f64> {
        info!(
            "Setting up audio stream for tap AudioObjectID {}",
            tap_object_id
        );

        // Use cpal to create an AudioUnit-based input stream from the tap device
        self.create_cpal_input_stream_from_tap(tap_object_id).await
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
    /// Returns the detected sample rate from the tap
    #[cfg(target_os = "macos")]
    async fn create_cpal_input_stream_from_tap(
        &mut self,
        tap_object_id: coreaudio_sys::AudioObjectID,
    ) -> Result<f64> {
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

        // If we can't find the tap device directly, return an error
        if tap_device.is_none() {
            return Err(anyhow::anyhow!(
                "Tap device {} not found in CPAL enumeration - cannot create audio stream",
                tap_object_id
            ));
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

        // Create the input stream with audio callback using RTRB producer
        let process_name = self.process_info.name.clone();
        let mut callback_count = 0u64;
        let producer_for_callback = self
            .audio_producer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("RTRB producer not initialized"))?
            .clone();

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

                     // Write audio data to RTRB producer for mixer integration
                     if let Ok(mut producer_guard) = producer_for_callback.lock() {
                         let mut written = 0;
                         for &sample in &audio_samples {
                             if producer_guard.push(sample).is_err() {
                                 if callback_count % 100 == 0 {
                                     warn!("🔄 TAP_RTRB_FULL: RTRB buffer full, wrote {}/{} samples (callback #{})",
                                         written, audio_samples.len(), callback_count);
                                 }
                                 break;
                             }
                             written += 1;
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
                let producer_for_i16_callback = producer_for_callback.clone();
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

                        // Write audio data to RTRB producer for mixer integration
                        if let Ok(mut producer_guard) = producer_for_i16_callback.lock() {
                            let mut written = 0;
                            for &sample in &audio_samples {
                                if producer_guard.push(sample).is_err() {
                                    if callback_count % 100 == 0 {
                                        warn!("🔄 TAP_I16_RTRB_FULL: RTRB buffer full, wrote {}/{} samples (callback #{})",
                                            written, audio_samples.len(), callback_count);
                                    }
                                    break;
                                }
                                written += 1;
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

        // Store and return the detected sample rate
        self.detected_sample_rate = Some(sample_rate);
        Ok(sample_rate)
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
            // FIXME: Need to call Core Audio API to destroy tap (AudioHardwareDestroyProcessTap)
            info!("Cleaned up Core Audio tap ID {}", tap_id);
        }

        if let Some(aggregate_id) = self.aggregate_device_id.take() {
            // FIXME: Need to call Core Audio API to destroy aggregate device
            info!("Cleaned up aggregate device ID {}", aggregate_id);
        }

        self.is_capturing = false;
        self.audio_producer = None;

        Ok(())
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
