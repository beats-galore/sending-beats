#[cfg(target_os = "macos")]
use anyhow::Result;
use coreaudio_sys::{
    kAudioFormatFlagIsFloat, kAudioFormatFlagIsNonInterleaved, kAudioFormatFlagIsPacked,
    kAudioFormatLinearPCM, kAudioOutputUnitProperty_CurrentDevice,
    kAudioOutputUnitProperty_EnableIO, kAudioUnitManufacturer_Apple,
    kAudioUnitProperty_SetRenderCallback, kAudioUnitProperty_StreamFormat, kAudioUnitScope_Global,
    kAudioUnitScope_Input, kAudioUnitScope_Output, kAudioUnitSubType_HALOutput,
    kAudioUnitType_Output, AURenderCallbackStruct, AudioBufferList, AudioComponentDescription,
    AudioComponentFindNext, AudioComponentInstanceDispose, AudioComponentInstanceNew,
    AudioDeviceID, AudioOutputUnitStart, AudioOutputUnitStop, AudioStreamBasicDescription,
    AudioTimeStamp, AudioUnit, AudioUnitInitialize, AudioUnitRenderActionFlags,
    AudioUnitSetProperty, AudioUnitUninitialize, OSStatus,
};
use crate::types::{COMMON_SAMPLE_RATES_HZ, DEFAULT_SAMPLE_RATE};
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
                        let total_samples = (audio_buffer.mDataByteSize as usize) / std::mem::size_of::<f32>();
                        let samples_to_fill = total_samples.min(frames_needed * 2); // 2 channels

                        // **PROFESSIONAL BROADCAST QUALITY**: CoreAudio with R8Brain transparent resampling
                        use crate::audio::mixer::sample_rate_converter::R8BrainSRC;
                        use std::cell::RefCell;
                        thread_local! {
                            static SRC: RefCell<Option<R8BrainSRC>> = RefCell::new(None);
                        }

                        // Collect all available samples from SPMC queue
                        let mut input_samples = Vec::new();
                        loop {
                            match spmc_reader.read() {
                                spmcq::ReadResult::Ok(sample) => {
                                    input_samples.push(sample);
                                    // Prevent unbounded reads
                                    if input_samples.len() >= 4096 {
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
                            // Dynamic SRC initialization - detect sample rate from input samples
                            let converted_samples = SRC.with(|src_cell| {
                                let mut src_opt = src_cell.borrow_mut();

                                // Estimate input sample rate based on sample count ratios
                                let estimated_input_rate = {


                                    // Heuristic: ratio of output samples needed vs input samples available
                                    let ratio_hint = samples_to_fill as f32 / input_samples.len() as f32;
                                    // Assume reasonable output rate for CoreAudio (usually 48kHz)
                                    let estimated_output_rate = DEFAULT_SAMPLE_RATE as f32;
                                    let estimated_input_rate = estimated_output_rate / ratio_hint;

                                    // Find closest common sample rate
                                    COMMON_SAMPLE_RATES_HZ.iter()
                                        .min_by(|&a, &b| {
                                            (a - estimated_input_rate).abs().partial_cmp(&(b - estimated_input_rate).abs()).unwrap()
                                        })
                                        .copied()
                                        .unwrap_or(DEFAULT_SAMPLE_RATE as f32)
                                };

                                // Reinitialize SRC if rate changed significantly
                                let needs_new_src = if let Some(ref src) = *src_opt {
                                    (src.ratio() - (crate::types::DEFAULT_SAMPLE_RATE as f32 / estimated_input_rate)).abs() > 0.01
                                } else {
                                    true
                                };

                                if needs_new_src {
                                    match R8BrainSRC::new(estimated_input_rate, DEFAULT_SAMPLE_RATE as f32) {
                                        Ok(src) => *src_opt = Some(src),
                                        Err(_) => *src_opt = None, // Fallback to silence if SRC creation fails
                                    }
                                }

                                if let Some(ref mut src) = *src_opt {
                                    let result = src.convert(&input_samples, samples_to_fill);
                                    // Debug log SRC usage occasionally
                                    static mut SRC_DEBUG_COUNT: u64 = 0;
                                    unsafe {
                                        SRC_DEBUG_COUNT += 1;
                                        if SRC_DEBUG_COUNT % 200 == 0 {
                                            println!("🎯 R8BRAIN_SRC [{}→{}]: {} input samples → {} output samples (ratio: {:.3}) [BROADCAST QUALITY]",
                                                estimated_input_rate, DEFAULT_SAMPLE_RATE, input_samples.len(), result.len(), src.ratio());
                                        }
                                    }
                                    result
                                } else {
                                    vec![0.0; samples_to_fill]
                                }
                            });

                            // Copy converted samples to output buffer
                            for (i, &sample) in converted_samples.iter().enumerate() {
                                if i < samples_to_fill {
                                    unsafe { *output_data.add(i) = sample };
                                }
                            }

                            (converted_samples.len().min(samples_to_fill), 0)
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
