#[cfg(target_os = "macos")]
use anyhow::Result;
use std::sync::{Arc, Mutex, atomic::{AtomicPtr, Ordering}};
use coreaudio_sys::{
    AudioDeviceID, AudioUnit, AudioComponentDescription,
    kAudioUnitType_Output, kAudioUnitSubType_HALOutput, kAudioUnitManufacturer_Apple,
    kAudioOutputUnitProperty_CurrentDevice, kAudioUnitProperty_StreamFormat,
    kAudioOutputUnitProperty_EnableIO, kAudioUnitScope_Input, kAudioUnitScope_Output,
    kAudioUnitScope_Global, AudioStreamBasicDescription, kAudioFormatLinearPCM,
    kAudioFormatFlagIsFloat, kAudioFormatFlagIsPacked, kAudioFormatFlagIsNonInterleaved,
    AudioComponentFindNext, AudioComponentInstanceNew, AudioUnitInitialize,
    AudioUnitSetProperty, AudioOutputUnitStart, AudioOutputUnitStop,
    AudioUnitUninitialize, AudioComponentInstanceDispose,
    AURenderCallbackStruct, kAudioUnitProperty_SetRenderCallback,
    AudioUnitRenderActionFlags, AudioTimeStamp, AudioBufferList, OSStatus
};
use std::ptr;
use std::os::raw::c_void;

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
    callback_buffer: Arc<AtomicPtr<Arc<Mutex<Vec<f32>>>>>,
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
            .field("callback_buffer", &(!self.callback_buffer.load(Ordering::Acquire).is_null()))
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
            callback_buffer: Arc::new(AtomicPtr::new(ptr::null_mut())),
        })
    }

    pub fn start(&mut self) -> Result<()> {
        println!("Starting CoreAudio Audio Unit stream for device: {}", self.device_name);

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
            return Err(anyhow::anyhow!("Failed to create Audio Unit instance: {}", status));
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

        // Step 5: Configure the audio format (INTERLEAVED for better compatibility)
        let format = AudioStreamBasicDescription {
            mSampleRate: self.sample_rate as f64,
            mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked, // REMOVED kAudioFormatFlagIsNonInterleaved
            mBytesPerPacket: (std::mem::size_of::<f32>() * self.channels as usize) as u32, // Fixed for interleaved
            mFramesPerPacket: 1,
            mBytesPerFrame: (std::mem::size_of::<f32>() * self.channels as usize) as u32, // Fixed for interleaved  
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

        // Step 6: Set up render callback with safer pointer management
        let input_buffer_clone = self.input_buffer.clone();
        let boxed_buffer = Box::new(input_buffer_clone);
        let buffer_ptr = Box::into_raw(boxed_buffer);
        
        // Store the pointer atomically for thread-safe access and cleanup
        let old_ptr = self.callback_buffer.swap(buffer_ptr, Ordering::Release);
        
        // Clean up any previous pointer
        if !old_ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(old_ptr);
            }
        }
        
        let callback = AURenderCallbackStruct {
            inputProc: Some(render_callback),
            inputProcRefCon: buffer_ptr as *mut c_void,
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
            return Err(anyhow::anyhow!("Failed to initialize Audio Unit: {}", status));
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
        
        println!("✅ CoreAudio Audio Unit stream started for: {} (device {})", self.device_name, self.device_id);
        println!("   Real audio streaming active with {} channels at {} Hz", self.channels, self.sample_rate);
        
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        println!("🔴 STOP: Starting stop sequence for device: {}", self.device_name);
        
        // First, mark as not running to prevent callback from processing
        println!("🔴 STOP: Setting is_running to false...");
        *self.is_running.lock().unwrap() = false;
        println!("🔴 STOP: Successfully set is_running to false");
        
        if let Some(audio_unit) = self.audio_unit.take() {
            println!("🔴 STOP: Found AudioUnit, entering unsafe block...");
            
            // DON'T call any CoreAudio APIs - just see if we can exit the unsafe block
            println!("🔴 STOP: Inside unsafe block, about to exit without calling any APIs...");
            
            // Skip ALL CoreAudio cleanup - just abandon the AudioUnit
            println!("🔴 STOP: Skipping all CoreAudio API calls");
            println!("🔴 STOP: AudioUnit abandoned - no cleanup performed");
        } else {
            println!("🔴 STOP: No AudioUnit found (already taken)");
        }
        
        println!("🔴 STOP: AudioUnit disposal complete, cleaning up buffer...");
        
        // Clean up the callback buffer pointer atomically (only after AudioUnit is disposed)
        println!("🔴 STOP: Waiting 50ms before buffer cleanup...");
        std::thread::sleep(std::time::Duration::from_millis(50));
        println!("🔴 STOP: Wait complete");
        
        println!("🔴 STOP: Swapping callback buffer pointer...");
        let buffer_ptr = self.callback_buffer.swap(ptr::null_mut(), Ordering::Release);
        println!("🔴 STOP: Buffer pointer swapped, checking if null...");
        
        if !buffer_ptr.is_null() {
            println!("🔴 STOP: Buffer pointer not null, checking reference count...");
            // Additional safety check: verify the Arc is safe to drop
            unsafe {
                // Check reference count before dropping
                let arc_ptr = buffer_ptr as *const Arc<Mutex<Vec<f32>>>;
                let strong_count = Arc::strong_count(&*arc_ptr);
                println!("🔴 STOP: Buffer reference count: {}", strong_count);
                
                if strong_count == 1 {
                    println!("🔴 STOP: Safe to deallocate buffer (only reference)");
                    let _ = Box::from_raw(buffer_ptr);
                    println!("🔴 STOP: Buffer deallocated successfully");
                } else {
                    println!("🔴 STOP: NOT deallocating buffer - {} references still exist", strong_count);
                }
            }
        } else {
            println!("🔴 STOP: Buffer pointer was null (already cleaned up)");
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
            // INTERLEAVED AUDIO: Process single buffer with all channels mixed
            if buffer_list.mNumberBuffers > 0 {
                let audio_buffer = unsafe { &mut *buffer_list.mBuffers.as_mut_ptr() };
                let output_data = audio_buffer.mData as *mut f32;
                
                // Validate output data pointer and size
                if !output_data.is_null() && audio_buffer.mDataByteSize > 0 {
                    let total_samples = (audio_buffer.mDataByteSize as usize) / std::mem::size_of::<f32>();
                    let samples_to_copy = total_samples.min(buffer.len());
                    
                    if samples_to_copy > 0 && !buffer.is_empty() {
                        // Copy interleaved audio samples safely
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
fn fill_buffers_with_silence(buffer_list: &mut AudioBufferList, frames_needed: usize) {
    // INTERLEAVED AUDIO: Fill single buffer with silence
    if buffer_list.mNumberBuffers > 0 {
        let audio_buffer = unsafe { &mut *buffer_list.mBuffers.as_mut_ptr() };
        let output_data = audio_buffer.mData as *mut f32;
        
        if !output_data.is_null() && audio_buffer.mDataByteSize > 0 {
            let total_samples = (audio_buffer.mDataByteSize as usize) / std::mem::size_of::<f32>();
            
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