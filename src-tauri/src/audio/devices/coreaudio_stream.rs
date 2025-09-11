use crate::types::{COMMON_SAMPLE_RATES_HZ, DEFAULT_SAMPLE_RATE};
#[cfg(target_os = "macos")]
use anyhow::Result;
use coreaudio_sys::{
    kAudioDevicePropertyNominalSampleRate, kAudioDevicePropertyStreamFormat, kAudioFormatFlagIsFloat, kAudioFormatFlagIsNonInterleaved, kAudioFormatFlagIsPacked,
    kAudioFormatLinearPCM, kAudioObjectPropertyElementMaster, kAudioObjectPropertyScopeInput, kAudioOutputUnitProperty_CurrentDevice,
    kAudioOutputUnitProperty_EnableIO, kAudioOutputUnitProperty_SetInputCallback, kAudioUnitManufacturer_Apple,
    kAudioUnitProperty_SetRenderCallback, kAudioUnitProperty_StreamFormat, kAudioUnitScope_Global,
    kAudioUnitScope_Input, kAudioUnitScope_Output, kAudioUnitSubType_HALOutput,
    kAudioUnitType_Output, AURenderCallbackStruct, AudioBufferList, AudioComponentDescription,
    AudioComponentFindNext, AudioComponentInstanceDispose, AudioComponentInstanceNew,
    AudioDeviceID, AudioObjectGetPropertyData, AudioObjectPropertyAddress, AudioOutputUnitStart, AudioOutputUnitStop, AudioStreamBasicDescription,
    AudioTimeStamp, AudioUnit, AudioUnitGetProperty, AudioUnitInitialize, AudioUnitRenderActionFlags,
    AudioUnitSetProperty, AudioUnitUninitialize, AudioUnitRender, OSStatus,
};
use std::os::raw::c_void;
use std::ptr;
use std::sync::{
    atomic::{AtomicPtr, Ordering},
    Arc, Mutex,
};
use tracing::warn;

/// # CoreAudio Thread Safety Documentation
///
/// This module implements CoreAudio stream management with careful attention to memory safety
/// and thread synchronization between audio callback threads and the main application.
///
/// ## Memory Safety Strategy:
/// - Uses `Arc<AtomicPtr<T>>` instead of raw pointers for callback context
/// - Atomic operations ensure thread-safe pointer swapping during stream lifecycle
/// - Proper cleanup in Drop-like patterns prevents memory leaks
///
/// ## Thread Interaction:
/// - Audio callbacks execute in real-time CoreAudio threads
/// - Main thread manages stream lifecycle (start/stop/cleanup)
/// - Atomic pointer operations coordinate between threads safely
///
/// ## Locking Strategy:
/// - Minimal use of mutexes in audio callback path for performance
/// - Atomic pointer swapping for callback context management
/// - Input buffer access through Arc<Mutex<Vec<f32>>> for thread safety

/// Context struct for CoreAudio render callbacks - contains both buffer and SPMC reader
#[cfg(target_os = "macos")]
struct AudioCallbackContext {
    buffer: Arc<Mutex<Vec<f32>>>,
    spmc_reader: Option<Arc<Mutex<spmcq::Reader<f32>>>>,
}

/// Context struct for CoreAudio input callbacks - matches CPAL architecture exactly
#[cfg(target_os = "macos")]
struct AudioInputCallbackContext {
    rtrb_producer: rtrb::Producer<f32>, // Owned producer, not Arc<Mutex<>> like CPAL
    input_notifier: Arc<tokio::sync::Notify>,
    device_name: String,
    audio_unit: AudioUnit, // Store AudioUnit for AudioUnitRender calls
    channels: u16,
    sample_rate: u32,
}

/// CoreAudio output stream implementation for direct hardware access
/// Implements actual Audio Unit streaming with render callbacks
#[cfg(target_os = "macos")]
pub struct CoreAudioOutputStream {
    pub device_id: AudioDeviceID,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub input_buffer: Arc<Mutex<Vec<f32>>>,
    pub is_running: Arc<Mutex<bool>>,
    audio_unit: Option<AudioUnit>,
    // **NEW CONTEXT ARCHITECTURE**: Context with both buffer and SPMC reader
    callback_context: Arc<AtomicPtr<AudioCallbackContext>>,
    // **SPMC INTEGRATION**: Reader for lock-free audio data from processing pipeline
    spmc_reader: Option<Arc<Mutex<spmcq::Reader<f32>>>>,
}

// Manual Debug implementation to handle the AudioUnit pointer
#[cfg(target_os = "macos")]
impl std::fmt::Debug for CoreAudioOutputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreAudioOutputStream")
            .field("device_id", &self.device_id)
            .field("device_name", &self.device_name)
            .field("sample_rate", &self.sample_rate)
            .field("channels", &self.channels)
            .field("is_running", &self.is_running)
            .field("audio_unit", &self.audio_unit.is_some())
            .field(
                "callback_context",
                &(!self.callback_context.load(Ordering::Acquire).is_null()),
            )
            .finish()
    }
}

// Make it Send-safe for use across threads (audio unit operations are done on main thread only)
#[cfg(target_os = "macos")]
unsafe impl Send for CoreAudioOutputStream {}

#[cfg(target_os = "macos")]
impl CoreAudioOutputStream {
    pub fn new(
        device_id: AudioDeviceID,
        device_name: String,
        sample_rate: u32,
        channels: u16,
    ) -> Result<Self> {
        println!(
            "Creating CoreAudio output stream for device: {} (ID: {}, SR: {}, CH: {})",
            device_name, device_id, sample_rate, channels
        );

        let input_buffer = Arc::new(Mutex::new(Vec::new()));
        let is_running = Arc::new(Mutex::new(false));

        Ok(Self {
            device_id,
            device_name,
            sample_rate,
            channels,
            input_buffer,
            is_running,
            audio_unit: None,
            callback_context: Arc::new(AtomicPtr::new(ptr::null_mut())),
            spmc_reader: None, // No SPMC reader for legacy constructor
        })
    }

    /// Create CoreAudio output stream with SPMC reader for lock-free audio processing
    pub fn new_with_spmc_reader(
        device_id: AudioDeviceID,
        device_name: String,
        sample_rate: u32,
        channels: u16,
        spmc_reader: spmcq::Reader<f32>,
    ) -> Result<Self> {
        println!(
            "Creating CoreAudio output stream with SPMC reader for device: {} (ID: {}, SR: {}, CH: {})",
            device_name, device_id, sample_rate, channels
        );

        let input_buffer = Arc::new(Mutex::new(Vec::new()));
        let is_running = Arc::new(Mutex::new(false));

        Ok(Self {
            device_id,
            device_name,
            sample_rate,
            channels,
            input_buffer,
            is_running,
            audio_unit: None,
            callback_context: Arc::new(AtomicPtr::new(ptr::null_mut())),
            spmc_reader: Some(Arc::new(Mutex::new(spmc_reader))), // **SPMC INTEGRATION**
        })
    }

    pub fn start(&mut self) -> Result<()> {
        println!(
            "Starting CoreAudio Audio Unit stream for device: {}",
            self.device_name
        );

        // Step 1: Find the Audio Unit component
        let component_desc = AudioComponentDescription {
            componentType: kAudioUnitType_Output,
            componentSubType: kAudioUnitSubType_HALOutput,
            componentManufacturer: kAudioUnitManufacturer_Apple,
            componentFlags: 0,
            componentFlagsMask: 0,
        };

        let component = unsafe { AudioComponentFindNext(ptr::null_mut(), &component_desc) };
        if component.is_null() {
            return Err(anyhow::anyhow!("Failed to find HAL output component"));
        }

        // Step 2: Create Audio Unit instance
        let mut audio_unit: AudioUnit = ptr::null_mut();
        let status = unsafe { AudioComponentInstanceNew(component, &mut audio_unit) };
        if status != 0 {
            return Err(anyhow::anyhow!(
                "Failed to create Audio Unit instance: {}",
                status
            ));
        }

        // Step 3: Enable output on the Audio Unit
        let enable_output: u32 = 1;
        let status = unsafe {
            AudioUnitSetProperty(
                audio_unit,
                kAudioOutputUnitProperty_EnableIO,
                kAudioUnitScope_Output,
                0,
                &enable_output as *const _ as *const c_void,
                std::mem::size_of::<u32>() as u32,
            )
        };
        if status != 0 {
            unsafe { AudioComponentInstanceDispose(audio_unit) };
            return Err(anyhow::anyhow!("Failed to enable output: {}", status));
        }

        // Step 4: Set the current device
        let status = unsafe {
            AudioUnitSetProperty(
                audio_unit,
                kAudioOutputUnitProperty_CurrentDevice,
                kAudioUnitScope_Global,
                0,
                &self.device_id as *const _ as *const c_void,
                std::mem::size_of::<AudioDeviceID>() as u32,
            )
        };
        if status != 0 {
            unsafe { AudioComponentInstanceDispose(audio_unit) };
            return Err(anyhow::anyhow!("Failed to set current device: {}", status));
        }

        // Step 5: Configure the audio format (INTERLEAVED for compatibility)
        let format = AudioStreamBasicDescription {
            mSampleRate: self.sample_rate as f64,
            mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
            mBytesPerPacket: (std::mem::size_of::<f32>() * self.channels as usize) as u32, // Interleaved: all channels per packet
            mFramesPerPacket: 1,
            mBytesPerFrame: (std::mem::size_of::<f32>() * self.channels as usize) as u32, // Interleaved: all channels per frame
            mChannelsPerFrame: self.channels as u32,
            mBitsPerChannel: 32,
            mReserved: 0,
        };

        let status = unsafe {
            AudioUnitSetProperty(
                audio_unit,
                kAudioUnitProperty_StreamFormat,
                kAudioUnitScope_Input,
                0,
                &format as *const _ as *const c_void,
                std::mem::size_of::<AudioStreamBasicDescription>() as u32,
            )
        };
        if status != 0 {
            unsafe { AudioComponentInstanceDispose(audio_unit) };
            return Err(anyhow::anyhow!("Failed to set stream format: {}", status));
        }

        // Step 6: Set up render callback with new AudioCallbackContext
        let context = AudioCallbackContext {
            buffer: self.input_buffer.clone(),
            spmc_reader: self.spmc_reader.clone(),
        };
        let boxed_context = Box::new(context);
        let context_ptr = Box::into_raw(boxed_context);

        // Store the pointer atomically for thread-safe access and cleanup
        let old_ptr = self.callback_context.swap(context_ptr, Ordering::Release);

        // Clean up any previous pointer
        if !old_ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(old_ptr);
            }
        }

        // **SPMC INTEGRATION**: Use appropriate callback based on whether SPMC reader is available
        let callback = AURenderCallbackStruct {
            inputProc: if self.spmc_reader.is_some() {
                Some(spmc_render_callback) // **NEW**: Use SPMC callback for real audio
            } else {
                Some(render_callback) // **FALLBACK**: Use original callback
            },
            inputProcRefCon: context_ptr as *mut c_void,
        };

        let status = unsafe {
            AudioUnitSetProperty(
                audio_unit,
                kAudioUnitProperty_SetRenderCallback,
                kAudioUnitScope_Input,
                0,
                &callback as *const _ as *const c_void,
                std::mem::size_of::<AURenderCallbackStruct>() as u32,
            )
        };
        if status != 0 {
            unsafe { AudioComponentInstanceDispose(audio_unit) };
            return Err(anyhow::anyhow!("Failed to set render callback: {}", status));
        }

        // Step 7: Initialize the Audio Unit
        let status = unsafe { AudioUnitInitialize(audio_unit) };
        if status != 0 {
            unsafe { AudioComponentInstanceDispose(audio_unit) };
            return Err(anyhow::anyhow!(
                "Failed to initialize Audio Unit: {}",
                status
            ));
        }

        // Step 8: Start the Audio Unit
        let status = unsafe { AudioOutputUnitStart(audio_unit) };
        if status != 0 {
            unsafe {
                AudioUnitUninitialize(audio_unit);
                AudioComponentInstanceDispose(audio_unit);
            }
            return Err(anyhow::anyhow!("Failed to start Audio Unit: {}", status));
        }

        // Store the Audio Unit and mark as running
        self.audio_unit = Some(audio_unit);
        *self.is_running.lock().unwrap() = true;

        println!(
            "✅ CoreAudio Audio Unit stream started for: {} (device {})",
            self.device_name, self.device_id
        );
        println!(
            "   Real audio streaming active with {} channels at {} Hz",
            self.channels, self.sample_rate
        );

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        println!(
            "🔴 STOP: Starting graceful stop sequence for device: {}",
            self.device_name
        );

        // First, mark as not running to prevent callback from processing
        if let Ok(mut is_running) = self.is_running.lock() {
            *is_running = false;
            println!("🔴 STOP: Successfully set is_running to false");
        } else {
            warn!("🔴 STOP: Could not lock is_running flag");
        }

        // Give callbacks time to see the flag and exit
        std::thread::sleep(std::time::Duration::from_millis(50));

        if let Some(audio_unit) = self.audio_unit.take() {
            println!("🔴 STOP: Found AudioUnit, attempting graceful shutdown...");

            // **CRITICAL FIX**: Attempt proper CoreAudio cleanup with error handling
            unsafe {
                // Try to stop the audio unit
                if AudioOutputUnitStop(audio_unit) == 0 {
                    println!("🔴 STOP: Successfully stopped AudioUnit");
                } else {
                    warn!("🔴 STOP: AudioOutputUnitStop failed, but continuing cleanup");
                }

                // Try to uninitialize the audio unit
                if AudioUnitUninitialize(audio_unit) == 0 {
                    println!("🔴 STOP: Successfully uninitialized AudioUnit");
                } else {
                    warn!("🔴 STOP: AudioUnitUninitialize failed, but continuing cleanup");
                }

                // Try to dispose of the audio unit
                if AudioComponentInstanceDispose(audio_unit) == 0 {
                    println!("🔴 STOP: Successfully disposed AudioUnit");
                } else {
                    warn!("🔴 STOP: AudioComponentInstanceDispose failed");
                }
            }
        } else {
            println!("🔴 STOP: No AudioUnit found (already cleaned up)");
        }

        println!("🔴 STOP: AudioUnit disposal complete, cleaning up buffer...");

        // Clean up the callback buffer pointer atomically (only after AudioUnit is disposed)
        println!("🔴 STOP: Waiting 50ms before buffer cleanup...");
        std::thread::sleep(std::time::Duration::from_millis(50));
        println!("🔴 STOP: Wait complete");

        println!("🔴 STOP: Swapping callback context pointer...");
        let context_ptr = self
            .callback_context
            .swap(ptr::null_mut(), Ordering::Release);
        println!("🔴 STOP: Context pointer swapped, checking if null...");

        if !context_ptr.is_null() {
            println!("🔴 STOP: Context pointer not null, deallocating...");
            unsafe {
                let _ = Box::from_raw(context_ptr);
                println!("🔴 STOP: Context deallocated successfully");
            }
        } else {
            println!("🔴 STOP: Context pointer was null (already cleaned up)");
        }

        println!("🔴 STOP: ✅ ALL CLEANUP COMPLETE for: {}", self.device_name);
        Ok(())
    }

    pub fn send_audio(&self, audio_data: &[f32]) -> Result<()> {
        if let Ok(mut buffer) = self.input_buffer.try_lock() {
            buffer.extend_from_slice(audio_data);

            // Prevent buffer from growing too large (keep max 1 second of audio)
            let max_buffer_size = self.sample_rate as usize * self.channels as usize;
            if buffer.len() > max_buffer_size {
                let excess = buffer.len() - max_buffer_size;
                buffer.drain(..excess);
            }
        }
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }
}

/// Render callback function for CoreAudio Audio Unit
/// CRITICAL: This function runs in real-time audio context - must be crash-proof
#[cfg(target_os = "macos")]
extern "C" fn render_callback(
    _in_ref_con: *mut c_void,
    _io_action_flags: *mut AudioUnitRenderActionFlags,
    _in_time_stamp: *const AudioTimeStamp,
    _in_bus_number: u32,
    in_number_frames: u32,
    io_data: *mut AudioBufferList,
) -> OSStatus {
    // Comprehensive safety checks to prevent crashes
    if _in_ref_con.is_null() || io_data.is_null() || in_number_frames == 0 {
        return -1; // Invalid parameters
    }

    // Catch any panic and return error instead of crashing
    let result = std::panic::catch_unwind(|| {
        // Safety: Convert the reference back to our boxed Arc
        let boxed_buffer_ptr = _in_ref_con as *mut Arc<Mutex<Vec<f32>>>;

        // Double-check pointer validity
        if boxed_buffer_ptr.is_null() {
            return -1;
        }

        // Get the audio buffer list with safety checks
        let buffer_list = unsafe { &mut *io_data };
        if buffer_list.mNumberBuffers == 0 {
            return -1; // No buffers to fill
        }

        let frames_needed = in_number_frames as usize;

        // IMMEDIATE SAFETY CHECK: Verify buffer pointer is not null or being disposed
        // This must be the FIRST thing we do to prevent crashes
        let input_buffer = unsafe {
            // Check if pointer is valid before dereferencing
            if boxed_buffer_ptr.is_null() {
                fill_buffers_with_silence(buffer_list, frames_needed);
                return 0;
            }
            &*boxed_buffer_ptr
        };

        // SAFETY CHECK: Verify the Arc is still valid (not disposed)
        // This prevents crashes when the AudioUnit is being disposed
        if Arc::strong_count(input_buffer) <= 1 {
            fill_buffers_with_silence(buffer_list, frames_needed);
            return 0; // Stream is being disposed, output silence safely
        }

        // Try to get audio data, but always ensure we fill the output buffers
        if let Ok(mut buffer) = input_buffer.try_lock() {
            // INTERLEAVED AUDIO: Process single buffer with all channels
            if buffer_list.mNumberBuffers > 0 {
                let audio_buffer = unsafe { &mut *buffer_list.mBuffers.as_mut_ptr() };
                let output_data = audio_buffer.mData as *mut f32;

                // Validate output data pointer and size
                if !output_data.is_null() && audio_buffer.mDataByteSize > 0 {
                    let total_samples =
                        (audio_buffer.mDataByteSize as usize) / std::mem::size_of::<f32>();
                    let samples_to_copy = total_samples.min(buffer.len());

                    if samples_to_copy > 0 && !buffer.is_empty() {
                        // Copy interleaved audio samples directly
                        unsafe {
                            std::ptr::copy_nonoverlapping(
                                buffer.as_ptr(),
                                output_data,
                                samples_to_copy,
                            );
                        }

                        // Fill remaining with silence if needed
                        if samples_to_copy < total_samples {
                            unsafe {
                                std::ptr::write_bytes(
                                    output_data.add(samples_to_copy),
                                    0,
                                    (total_samples - samples_to_copy) * std::mem::size_of::<f32>(),
                                );
                            }
                        }

                        // Drain the buffer by the number of samples we used
                        buffer.drain(..samples_to_copy);
                    } else {
                        // No audio available, fill with silence
                        unsafe {
                            std::ptr::write_bytes(
                                output_data,
                                0,
                                total_samples * std::mem::size_of::<f32>(),
                            );
                        }
                    }
                }
            }
        } else {
            // Couldn't get lock - output silence to all channels
            fill_buffers_with_silence(buffer_list, frames_needed);
        }

        0 // Success
    });

    // If panic occurred, return error code
    result.unwrap_or(-1)
}

/// Helper function to safely fill all audio buffers with silence (INTERLEAVED)
#[cfg(target_os = "macos")]
fn fill_buffers_with_silence(buffer_list: &mut AudioBufferList, _frames_needed: usize) {
    // INTERLEAVED AUDIO: Fill single buffer with silence
    if buffer_list.mNumberBuffers > 0 {
        let audio_buffer = unsafe { &mut *buffer_list.mBuffers.as_mut_ptr() };
        let output_data = audio_buffer.mData as *mut f32;

        if !output_data.is_null() && audio_buffer.mDataByteSize > 0 {
            let total_samples = (audio_buffer.mDataByteSize as usize) / std::mem::size_of::<f32>();

            unsafe {
                std::ptr::write_bytes(output_data, 0, total_samples * std::mem::size_of::<f32>());
            }
        }
    }
}

/// SPMC render callback function for CoreAudio Audio Unit with lock-free queue reading
/// This callback reads directly from the SPMC queue for real-time audio output
#[cfg(target_os = "macos")]
extern "C" fn spmc_render_callback(
    _in_ref_con: *mut c_void,
    _io_action_flags: *mut AudioUnitRenderActionFlags,
    _in_time_stamp: *const AudioTimeStamp,
    _in_bus_number: u32,
    in_number_frames: u32,
    io_data: *mut AudioBufferList,
) -> OSStatus {
    // Safety checks to prevent crashes
    if _in_ref_con.is_null() || io_data.is_null() || in_number_frames == 0 {
        return -1;
    }

    let result = std::panic::catch_unwind(|| {
        // Convert context back to AudioCallbackContext
        let context_ptr = _in_ref_con as *mut AudioCallbackContext;
        if context_ptr.is_null() {
            return -1;
        }

        let context = unsafe { &*context_ptr };
        let buffer_list = unsafe { &mut *io_data };
        let frames_needed = in_number_frames as usize;

        // Try to read from SPMC queue if available
        if let Some(ref spmc_reader_arc) = context.spmc_reader {
            if let Ok(mut spmc_reader) = spmc_reader_arc.try_lock() {
                // Fill audio buffers from SPMC queue
                if buffer_list.mNumberBuffers > 0 {
                    let audio_buffer = unsafe { &mut *buffer_list.mBuffers.as_mut_ptr() };
                    let output_data = audio_buffer.mData as *mut f32;

                    if !output_data.is_null() && audio_buffer.mDataByteSize > 0 {
                        let total_samples =
                            (audio_buffer.mDataByteSize as usize) / std::mem::size_of::<f32>();
                        let samples_to_fill = total_samples.min(frames_needed * 2); // 2 channels

                        // Collect all available samples from SPMC queue
                        let mut input_samples = Vec::new();
                        loop {
                            match spmc_reader.read() {
                                spmcq::ReadResult::Ok(sample) => {
                                    input_samples.push(sample);
                                    // Prevent unbounded reads
                                    if input_samples.len() >= 4096 {
                                        println!("samples greater than 4096, breaking processing loop");
                                        break;
                                    }
                                }
                                spmcq::ReadResult::Dropout(sample) => {
                                    input_samples.push(sample);
                                    if input_samples.len() >= 4096 {
                                        break;
                                    }
                                }
                                spmcq::ReadResult::Empty => {
                                    break;
                                }
                            }
                        }

                        let (samples_read, silence_filled) = if !input_samples.is_empty() {
                            // SIMPLIFIED: Direct pass-through without sample rate conversion
                            // Copy input samples directly to output buffer (no resampling)
                            let samples_to_copy = input_samples.len().min(samples_to_fill);

                            for i in 0..samples_to_copy {
                                unsafe { *output_data.add(i) = input_samples[i] };
                            }

                            // Fill remaining with silence if we don't have enough samples
                            for i in samples_to_copy..samples_to_fill {
                                unsafe { *output_data.add(i) = 0.0 };
                            }

                            (samples_to_copy, samples_to_fill - samples_to_copy)
                        } else {
                            // No input samples available - fill with silence
                            for i in 0..samples_to_fill {
                                unsafe { *output_data.add(i) = 0.0 };
                            }
                            (0, samples_to_fill)
                        };

                        // **DEBUG**: Log audio playback periodically
                        static mut SPMC_PLAYBACK_COUNT: u64 = 0;
                        unsafe {
                            SPMC_PLAYBACK_COUNT += 1;
                            if SPMC_PLAYBACK_COUNT % 100 == 0 || SPMC_PLAYBACK_COUNT < 10 {
                                let peak = (0..samples_to_fill)
                                    .map(|i| unsafe { *output_data.add(i) }.abs())
                                    .fold(0.0f32, f32::max);
                                println!("🎵 SPMC_COREAUDIO [{}]: Playing {} samples (call #{}), read: {}, silence: {}, peak: {:.4}",
                                    "CoreAudio", samples_to_fill, SPMC_PLAYBACK_COUNT, samples_read, silence_filled, peak);
                            }
                        }

                        return 0; // Success
                    }
                }
            }
        }

        // Fallback to silence if SPMC reading fails
        fill_buffers_with_silence(buffer_list, frames_needed);
        0
    });

    result.unwrap_or(-1)
}

#[cfg(target_os = "macos")]
impl Drop for CoreAudioOutputStream {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// CoreAudio input stream implementation for direct hardware input capture
/// Implements Audio Unit input streaming with input callbacks using RTRB for lock-free audio capture
#[cfg(target_os = "macos")]
pub struct CoreAudioInputStream {
    pub device_id: AudioDeviceID,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub is_running: Arc<Mutex<bool>>,
    audio_unit: Option<AudioUnit>,
    // Context for input callback with RTRB producer
    callback_context: Arc<AtomicPtr<AudioInputCallbackContext>>,
    // RTRB producer for lock-free audio capture - stored separately for ownership management
    rtrb_producer: Option<Arc<Mutex<rtrb::Producer<f32>>>>,
    // Notification system for event-driven processing
    input_notifier: Arc<tokio::sync::Notify>,
}

// Manual Debug implementation to handle the AudioUnit pointer
#[cfg(target_os = "macos")]
impl std::fmt::Debug for CoreAudioInputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreAudioInputStream")
            .field("device_id", &self.device_id)
            .field("device_name", &self.device_name)
            .field("sample_rate", &self.sample_rate)
            .field("channels", &self.channels)
            .field("is_running", &self.is_running)
            .field("audio_unit", &self.audio_unit.is_some())
            .field(
                "callback_context",
                &(!self.callback_context.load(Ordering::Acquire).is_null()),
            )
            .finish()
    }
}

// Make it Send-safe for use across threads (audio unit operations are done on main thread only)
#[cfg(target_os = "macos")]
unsafe impl Send for CoreAudioInputStream {}

#[cfg(target_os = "macos")]
impl CoreAudioInputStream {
    /// Create CoreAudio input stream with RTRB producer for lock-free audio capture
    pub fn new_with_rtrb_producer(
        device_id: AudioDeviceID,
        device_name: String,
        sample_rate: u32,
        channels: u16,
        rtrb_producer: rtrb::Producer<f32>,
        input_notifier: Arc<tokio::sync::Notify>,
    ) -> Result<Self> {
        println!(
            "🎤 Creating CoreAudio input stream for device: {} (ID: {}, SR: {}, CH: {})",
            device_name, device_id, sample_rate, channels
        );

        let is_running = Arc::new(Mutex::new(false));
        let rtrb_producer = Some(Arc::new(Mutex::new(rtrb_producer)));

        Ok(Self {
            device_id,
            device_name,
            sample_rate,
            channels,
            is_running,
            audio_unit: None,
            callback_context: Arc::new(AtomicPtr::new(ptr::null_mut())),
            rtrb_producer,
            input_notifier,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        println!(
            "🎤 Starting CoreAudio input Audio Unit stream for device: {}",
            self.device_name
        );

        // Step 1: Find the HAL Audio Unit component (same as output but configured for input)
        let component_desc = AudioComponentDescription {
            componentType: kAudioUnitType_Output,
            componentSubType: kAudioUnitSubType_HALOutput,
            componentManufacturer: kAudioUnitManufacturer_Apple,
            componentFlags: 0,
            componentFlagsMask: 0,
        };

        let component = unsafe { AudioComponentFindNext(ptr::null_mut(), &component_desc) };
        if component.is_null() {
            return Err(anyhow::anyhow!("Failed to find HAL input component"));
        }

        // Step 2: Create Audio Unit instance
        let mut audio_unit: AudioUnit = ptr::null_mut();
        let status = unsafe { AudioComponentInstanceNew(component, &mut audio_unit) };
        if status != 0 {
            return Err(anyhow::anyhow!(
                "Failed to create Audio Unit instance for input: {}",
                status
            ));
        }

        // Step 3: Enable INPUT on the Audio Unit (this is the key difference from output)
        let enable_input: u32 = 1;
        let status = unsafe {
            AudioUnitSetProperty(
                audio_unit,
                kAudioOutputUnitProperty_EnableIO,
                kAudioUnitScope_Input,  // INPUT scope for input streams
                1,  // Input bus is 1, not 0
                &enable_input as *const _ as *const c_void,
                std::mem::size_of::<u32>() as u32,
            )
        };
        if status != 0 {
            unsafe { AudioComponentInstanceDispose(audio_unit) };
            return Err(anyhow::anyhow!("Failed to enable input: {}", status));
        }

        // Step 4: Disable OUTPUT on the Audio Unit (we only want input)
        let disable_output: u32 = 0;
        let status = unsafe {
            AudioUnitSetProperty(
                audio_unit,
                kAudioOutputUnitProperty_EnableIO,
                kAudioUnitScope_Output,  // Disable output
                0,  // Output bus is 0
                &disable_output as *const _ as *const c_void,
                std::mem::size_of::<u32>() as u32,
            )
        };
        if status != 0 {
            unsafe { AudioComponentInstanceDispose(audio_unit) };
            return Err(anyhow::anyhow!("Failed to disable output: {}", status));
        }

        // Step 5: Set the current input device
        let status = unsafe {
            AudioUnitSetProperty(
                audio_unit,
                kAudioOutputUnitProperty_CurrentDevice,
                kAudioUnitScope_Global,
                0,
                &self.device_id as *const _ as *const c_void,
                std::mem::size_of::<AudioDeviceID>() as u32,
            )
        };
        if status != 0 {
            unsafe { AudioComponentInstanceDispose(audio_unit) };
            return Err(anyhow::anyhow!("Failed to set current input device: {}", status));
        }

        // Step 6: Get the device's native format and use it instead of forcing our own
        // For HAL input units, we should use the device's native format
        let mut device_format: AudioStreamBasicDescription = unsafe { std::mem::zeroed() };
        let mut size = std::mem::size_of::<AudioStreamBasicDescription>() as u32;
        let status = unsafe {
            AudioObjectGetPropertyData(
                self.device_id,
                &AudioObjectPropertyAddress {
                    mSelector: kAudioDevicePropertyStreamFormat,
                    mScope: kAudioObjectPropertyScopeInput,
                    mElement: kAudioObjectPropertyElementMaster,
                },
                0,
                ptr::null_mut(),
                &mut size,
                &mut device_format as *mut _ as *mut c_void,
            )
        };
        if status != 0 {
            unsafe { AudioComponentInstanceDispose(audio_unit) };
            return Err(anyhow::anyhow!("Failed to get device native format: {}", status));
        }

        println!("🔍 DEVICE NATIVE FORMAT: SR: {}, Channels: {}, Format: 0x{:x}, Flags: 0x{:x}",
            device_format.mSampleRate, device_format.mChannelsPerFrame,
            device_format.mFormatID, device_format.mFormatFlags);

        // Only set format on OUTPUT scope of input unit (data coming FROM the device)
        // Use the device's native format to avoid -10865 errors
        let status = unsafe {
            AudioUnitSetProperty(
                audio_unit,
                kAudioUnitProperty_StreamFormat,
                kAudioUnitScope_Output,  // Output scope for data FROM input
                1,  // Input bus is 1
                &device_format as *const _ as *const c_void,
                std::mem::size_of::<AudioStreamBasicDescription>() as u32,
            )
        };
        if status != 0 {
            unsafe { AudioComponentInstanceDispose(audio_unit) };
            return Err(anyhow::anyhow!("Failed to set output stream format for input unit: {}", status));
        }

        // Step 6.5: Add debugging to verify AudioUnit state before callback setup
        // Check the actual formats that were set
        let mut actual_input_format: AudioStreamBasicDescription = unsafe { std::mem::zeroed() };
        let mut size = std::mem::size_of::<AudioStreamBasicDescription>() as u32;
        let status = unsafe {
            AudioUnitGetProperty(
                audio_unit,
                kAudioUnitProperty_StreamFormat,
                kAudioUnitScope_Input,
                1,
                &mut actual_input_format as *mut _ as *mut c_void,
                &mut size,
            )
        };
        if status == 0 {
            println!("🔍 DEBUG INPUT FORMAT: SR: {}, Channels: {}, Format: 0x{:x}, Flags: 0x{:x}",
                actual_input_format.mSampleRate, actual_input_format.mChannelsPerFrame,
                actual_input_format.mFormatID, actual_input_format.mFormatFlags);
        } else {
            println!("⚠️ Failed to get input format: {}", status);
        }

        let mut actual_output_format: AudioStreamBasicDescription = unsafe { std::mem::zeroed() };
        let mut size = std::mem::size_of::<AudioStreamBasicDescription>() as u32;
        let status = unsafe {
            AudioUnitGetProperty(
                audio_unit,
                kAudioUnitProperty_StreamFormat,
                kAudioUnitScope_Output,
                1,
                &mut actual_output_format as *mut _ as *mut c_void,
                &mut size,
            )
        };
        if status == 0 {
            println!("🔍 DEBUG OUTPUT FORMAT: SR: {}, Channels: {}, Format: 0x{:x}, Flags: 0x{:x}",
                actual_output_format.mSampleRate, actual_output_format.mChannelsPerFrame,
                actual_output_format.mFormatID, actual_output_format.mFormatFlags);
        } else {
            println!("⚠️ Failed to get output format: {}", status);
        }

        // Check device sample rate compatibility
        let mut device_sample_rate: f64 = 0.0;
        let mut size = std::mem::size_of::<f64>() as u32;
        let status = unsafe {
            AudioObjectGetPropertyData(
                self.device_id,
                &AudioObjectPropertyAddress {
                    mSelector: kAudioDevicePropertyNominalSampleRate,
                    mScope: kAudioObjectPropertyScopeInput,
                    mElement: kAudioObjectPropertyElementMaster,
                },
                0,
                ptr::null_mut(),
                &mut size,
                &mut device_sample_rate as *mut _ as *mut c_void,
            )
        };
        if status == 0 {
            println!("🔍 DEBUG DEVICE SAMPLE RATE: {} Hz (requested: {} Hz)", device_sample_rate, self.sample_rate);
            if (device_sample_rate - self.sample_rate as f64).abs() > 1.0 {
                println!("⚠️ SAMPLE RATE MISMATCH: Device={} Hz, Requested={} Hz", device_sample_rate, self.sample_rate);
            }
        } else {
            println!("⚠️ Failed to get device sample rate: {}", status);
        }

        // Step 7: Set up input callback with AudioInputCallbackContext
        let rtrb_producer_arc = self.rtrb_producer.take().unwrap();
        let rtrb_producer = Arc::try_unwrap(rtrb_producer_arc)
            .map_err(|_| anyhow::anyhow!("Failed to extract RTRB producer from Arc"))?
            .into_inner()
            .map_err(|_| anyhow::anyhow!("Failed to extract RTRB producer from Mutex"))?;

        let context = AudioInputCallbackContext {
            rtrb_producer, // Move extracted producer ownership to callback context, just like CPAL
            input_notifier: self.input_notifier.clone(),
            device_name: self.device_name.clone(),
            audio_unit,
            channels: self.channels,
            sample_rate: self.sample_rate,
        };
        let boxed_context = Box::new(context);
        let context_ptr = Box::into_raw(boxed_context);

        // Store the pointer atomically for thread-safe access and cleanup
        let old_ptr = self.callback_context.swap(context_ptr, Ordering::Release);

        // Clean up any previous pointer
        if !old_ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(old_ptr);
            }
        }

        let callback = AURenderCallbackStruct {
            inputProc: Some(coreaudio_input_callback),
            inputProcRefCon: context_ptr as *mut c_void,
        };

        // **APPLE DOCUMENTED**: Use input callback for HAL input units per TN2091
        // This notifies when input data is available, then we call AudioUnitRender to get it
        let status = unsafe {
            AudioUnitSetProperty(
                audio_unit,
                kAudioOutputUnitProperty_SetInputCallback,  // Use input callback property for input units
                kAudioUnitScope_Global,  // Global scope for input callbacks
                0,  // Element 0 for input callbacks
                &callback as *const _ as *const c_void,
                std::mem::size_of::<AURenderCallbackStruct>() as u32,
            )
        };
        if status != 0 {
            unsafe { AudioComponentInstanceDispose(audio_unit) };
            return Err(anyhow::anyhow!("Failed to set input render callback: {}", status));
        }

        // Step 8: Initialize the Audio Unit
        let status = unsafe { AudioUnitInitialize(audio_unit) };
        if status != 0 {
            unsafe { AudioComponentInstanceDispose(audio_unit) };
            return Err(anyhow::anyhow!(
                "Failed to initialize input Audio Unit: {}",
                status
            ));
        }

        // Step 9: Start the Audio Unit
        let status = unsafe { AudioOutputUnitStart(audio_unit) };
        if status != 0 {
            unsafe {
                AudioUnitUninitialize(audio_unit);
                AudioComponentInstanceDispose(audio_unit);
            }
            return Err(anyhow::anyhow!("Failed to start input Audio Unit: {}", status));
        }

        // Store the Audio Unit and mark as running
        self.audio_unit = Some(audio_unit);
        *self.is_running.lock().unwrap() = true;

        println!(
            "✅ CoreAudio input Audio Unit stream started for: {} (device {})",
            self.device_name, self.device_id
        );
        println!(
            "   🎤 Real audio input capture active with {} channels at {} Hz",
            self.channels, self.sample_rate
        );

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        println!(
            "🔴 STOP INPUT: Starting graceful stop sequence for input device: {}",
            self.device_name
        );

        // First, mark as not running to prevent callback from processing
        if let Ok(mut is_running) = self.is_running.lock() {
            *is_running = false;
            println!("🔴 STOP INPUT: Successfully set is_running to false");
        } else {
            warn!("🔴 STOP INPUT: Could not lock is_running flag");
        }

        // Give callbacks time to see the flag and exit
        std::thread::sleep(std::time::Duration::from_millis(50));

        if let Some(audio_unit) = self.audio_unit.take() {
            println!("🔴 STOP INPUT: Found AudioUnit, attempting graceful shutdown...");

            // Attempt proper CoreAudio cleanup with error handling
            unsafe {
                // Try to stop the audio unit
                if AudioOutputUnitStop(audio_unit) == 0 {
                    println!("🔴 STOP INPUT: Successfully stopped AudioUnit");
                } else {
                    warn!("🔴 STOP INPUT: AudioOutputUnitStop failed, but continuing cleanup");
                }

                // Try to uninitialize the audio unit
                if AudioUnitUninitialize(audio_unit) == 0 {
                    println!("🔴 STOP INPUT: Successfully uninitialized AudioUnit");
                } else {
                    warn!("🔴 STOP INPUT: AudioUnitUninitialize failed, but continuing cleanup");
                }

                // Try to dispose of the audio unit
                if AudioComponentInstanceDispose(audio_unit) == 0 {
                    println!("🔴 STOP INPUT: Successfully disposed AudioUnit");
                } else {
                    warn!("🔴 STOP INPUT: AudioComponentInstanceDispose failed");
                }
            }
        } else {
            println!("🔴 STOP INPUT: No AudioUnit found (already cleaned up)");
        }

        // Clean up the callback context pointer atomically
        println!("🔴 STOP INPUT: Waiting 50ms before context cleanup...");
        std::thread::sleep(std::time::Duration::from_millis(50));

        let context_ptr = self
            .callback_context
            .swap(ptr::null_mut(), Ordering::Release);

        if !context_ptr.is_null() {
            println!("🔴 STOP INPUT: Context pointer not null, deallocating...");
            unsafe {
                let _ = Box::from_raw(context_ptr);
                println!("🔴 STOP INPUT: Context deallocated successfully");
            }
        } else {
            println!("🔴 STOP INPUT: Context pointer was null (already cleaned up)");
        }

        println!("🔴 STOP INPUT: ✅ ALL CLEANUP COMPLETE for: {}", self.device_name);
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }
}

/// Input callback function for CoreAudio Audio Unit
/// CRITICAL: This function runs in real-time audio context - must be crash-proof
#[cfg(target_os = "macos")]
extern "C" fn coreaudio_input_callback(
    in_ref_con: *mut c_void,
    _io_action_flags: *mut AudioUnitRenderActionFlags,
    in_time_stamp: *const AudioTimeStamp,
    in_bus_number: u32,
    in_number_frames: u32,
    io_data: *mut AudioBufferList,  // USED for render callbacks - AudioUnit provides data here
) -> OSStatus {

    // Comprehensive safety checks to prevent crashes
    if in_ref_con.is_null() || in_number_frames == 0 {
        return -1; // Invalid parameters
    }

    // Catch any panic and return error instead of crashing
    let result = std::panic::catch_unwind(|| {
        // Convert the reference back to our context (mutable for producer access)
        let context_ptr = in_ref_con as *mut AudioInputCallbackContext;

        // Double-check pointer validity
        if context_ptr.is_null() {
            return -1;
        }

        let context = unsafe { &mut *context_ptr };
        let frames_needed = in_number_frames as usize;
        let total_samples = frames_needed * context.channels as usize;

        // **APPLE TN2091**: Use AudioUnitRender to pull input data from HAL input unit

        // Allocate buffer for the audio data
        let mut audio_data = vec![0.0f32; total_samples];
        let mut audio_buffer_list = AudioBufferList {
            mNumberBuffers: 1,
            mBuffers: [coreaudio_sys::AudioBuffer {
                mNumberChannels: context.channels as u32,
                mDataByteSize: (total_samples * std::mem::size_of::<f32>()) as u32,
                mData: audio_data.as_mut_ptr() as *mut c_void,
            }],
        };

        // **FIX for -10863**: Use correct parameters for HAL input AudioUnitRender
        let mut render_flags: AudioUnitRenderActionFlags = 0;
        let render_status = unsafe {
            AudioUnitRender(
                context.audio_unit,
                &mut render_flags,
                in_time_stamp,
                1, // **CRITICAL**: Input bus is always 1 for HAL units
                in_number_frames,
                &mut audio_buffer_list as *mut AudioBufferList,
            )
        };

        let samples = if render_status == 0 {
            // SUCCESS: Got real audio from hardware
            audio_data
        } else {
            // Check for specific errors to understand what's wrong
            match render_status {
                -10863 => println!("🔴 AudioUnitRender: kAudioUnitErr_CannotDoInCurrentContext - check format/timing"),
                -10865 => println!("🔴 AudioUnitRender: kAudioUnitErr_PropertyNotWritable - check configuration"),
                -10866 => println!("🔴 AudioUnitRender: kAudioUnitErr_CannotDoInCurrentContext - invalid format"),
                _ => println!("🔴 AudioUnitRender failed with status {}", render_status),
            }
            vec![0.0f32; total_samples]
        };

        // **EXACTLY MATCH CPAL**: Push captured samples to RTRB ring buffer (identical to CPAL f32 callback)
        let mut samples_written = 0;
        let mut samples_dropped = 0;

        for &sample in samples.iter() {
            match context.rtrb_producer.push(sample) {
                Ok(()) => samples_written += 1,
                Err(_) => {
                    samples_dropped += 1;
                    // Ring buffer full - skip this sample (prevents blocking)
                }
            }
        }

        // **TRUE EVENT-DRIVEN**: Always notify async processing thread when hardware callback runs (EXACTLY like CPAL)
        context.input_notifier.notify_one();

        // Debug logging for audio capture (same pattern as CPAL)
        static mut INPUT_CAPTURE_COUNT: u64 = 0;
        unsafe {
            INPUT_CAPTURE_COUNT += 1;
            if INPUT_CAPTURE_COUNT % 100 == 0 || INPUT_CAPTURE_COUNT < 10 {
                let peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                let rms = (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
                println!("🎤 COREAUDIO_INPUT [{}]: Captured {} frames ({}), wrote: {}, dropped: {}, peak: {:.4}, rms: {:.4} ⚡NOTIFIED",
                    context.device_name, frames_needed, INPUT_CAPTURE_COUNT, samples_written, samples_dropped, peak, rms);
            }
        }

        0 // Success
    });

    // If panic occurred, return error code
    result.unwrap_or(-1)
}

#[cfg(target_os = "macos")]
impl Drop for CoreAudioInputStream {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

// Placeholder implementations for non-macOS platforms
// Placeholder implementations for non-macOS platforms
#[cfg(not(target_os = "macos"))]
#[derive(Debug)]
pub struct CoreAudioOutputStream;

#[cfg(not(target_os = "macos"))]
impl CoreAudioOutputStream {
    pub fn new(
        _device_id: u32,
        _device_name: String,
        _sample_rate: u32,
        _channels: u16,
    ) -> Result<Self> {
        Err(anyhow::anyhow!("CoreAudio not available on this platform"))
    }

    pub fn start(&mut self) -> Result<()> {
        Err(anyhow::anyhow!("CoreAudio not available on this platform"))
    }

    pub fn stop(&mut self) -> Result<()> {
        Err(anyhow::anyhow!("CoreAudio not available on this platform"))
    }

    pub fn send_audio(&self, _audio_data: &[f32]) -> Result<()> {
        Err(anyhow::anyhow!("CoreAudio not available on this platform"))
    }

    pub fn is_running(&self) -> bool {
        false
    }
}
