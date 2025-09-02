// Custom FFI bindings for macOS Core Audio Taps API (14.4+)
// These are not yet available in coreaudio-sys, so we define them manually

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::ffi::CStr;
use std::os::raw::{c_char, c_void};

// Core Audio Types from coreaudio-sys
pub use coreaudio_sys::{
    kAudioObjectSystemObject, AudioObjectGetPropertyData, AudioObjectID,
    AudioObjectPropertyAddress, AudioStreamBasicDescription, Float64, OSStatus, UInt32,
};

// Modern Core Audio Process Taps from objc2_core_audio
#[cfg(target_os = "macos")]
pub use objc2_core_audio::{
    AudioHardwareCreateProcessTap, AudioHardwareDestroyProcessTap, CATapDescription,
};

// Additional imports for Objective-C objects
#[cfg(target_os = "macos")]
pub use objc2::runtime::AnyClass;
#[cfg(target_os = "macos")]
pub use objc2::AnyThread;
#[cfg(target_os = "macos")]
pub use objc2_foundation::{NSArray, NSNumber};

// Process tap constants and types
pub const kAudioTapDescription_ProcessArray_Size: usize = 8;
pub const kAudioTapFormatType_Process: u32 = 1886613872; // 'proc'
pub const kAudioTapFormatType_Output: u32 = 1869968244; // 'outp'

pub const kAudioHardwarePropertyTranslatePIDToProcessObject: u32 = 1886352239; // 'pidx'

// CATapDescription is now provided by objc2_core_audio

/// Helper function to create a CATapDescription for a specific process using PID
#[cfg(target_os = "macos")]
pub fn create_process_tap_description(pid: u32) -> objc2::rc::Retained<CATapDescription> {
    use objc2::rc::Retained;
    use objc2::{runtime::AnyClass, ClassType};
    use tracing::info;

    unsafe {
        // BREAKTHROUGH: Create process-specific tap using initStereoMixdownOfProcesses
        // This should capture ONLY the specified process (Apple Music)

        info!("Creating process-specific tap for PID: {}", pid);

        let alloc = CATapDescription::alloc();

        // Create NSNumber for the PID and put it in an NSArray
        let pid_number = NSNumber::new_u32(pid);
        let process_array = NSArray::from_slice(&[&*pid_number]);

        info!(
            "Created process array with PID {} for process-specific tap",
            pid
        );

        // Use initStereoMixdownOfProcesses to create a tap that captures ONLY this process
        CATapDescription::initStereoMixdownOfProcesses(alloc, &process_array)
    }
}

/// Aggregate device description for creating virtual audio devices
#[repr(C)]
#[derive(Debug)]
pub struct AudioAggregateDeviceDescription {
    pub device_name: *const c_char,
    pub device_uid: *const c_char,
    pub sub_device_list: *const AudioObjectID,
    pub number_sub_devices: UInt32,
    pub sample_rate: Float64,
    pub is_private: bool,
}

// AudioHardwareCreateProcessTap and AudioHardwareDestroyProcessTap are now provided by objc2_core_audio

extern "C" {
    /// Create an aggregate device from multiple audio sources
    /// This has been available longer but we use it with taps
    pub fn AudioHardwareCreateAggregateDevice(
        in_description: *const AudioAggregateDeviceDescription,
        out_device_object_id: *mut AudioObjectID,
    ) -> OSStatus;

    /// Destroy an aggregate device
    pub fn AudioHardwareDestroyAggregateDevice(in_device_object_id: AudioObjectID) -> OSStatus;

    /// Get property from Audio Hardware
    /// We use this to translate PID to AudioObjectID
    pub fn AudioHardwareGetProperty(
        in_address: *const AudioObjectPropertyAddress,
        io_data_size: *mut UInt32,
        out_data: *mut c_void,
    ) -> OSStatus;

    /// Set property on Audio Hardware  
    pub fn AudioHardwareSetProperty(
        in_address: *const AudioObjectPropertyAddress,
        in_data_size: UInt32,
        in_data: *const c_void,
    ) -> OSStatus;
}

/// Check if AudioHardwareCreateProcessTap function is available at runtime
pub fn is_process_tap_available() -> bool {
    use std::ffi::CString;

    unsafe {
        // Try multiple frameworks where the function might be located
        let frameworks = ["AudioToolbox", "CoreAudio", "AudioUnit"];

        for framework in &frameworks {
            let lib_name = CString::new(*framework).unwrap();
            let lib_handle = libc::dlopen(lib_name.as_ptr(), libc::RTLD_LAZY);
            if lib_handle.is_null() {
                continue;
            }

            let func_name = CString::new("AudioHardwareCreateProcessTap").unwrap();
            let func_ptr = libc::dlsym(lib_handle, func_name.as_ptr());
            libc::dlclose(lib_handle);

            if !func_ptr.is_null() {
                tracing::info!(
                    "Found AudioHardwareCreateProcessTap in framework: {}",
                    framework
                );
                return true;
            }
        }

        tracing::warn!("AudioHardwareCreateProcessTap not found in any framework");
        false
    }
}

/// Safe wrapper for AudioHardwareCreateProcessTap using objc2_core_audio
pub unsafe fn create_process_tap(
    description: &objc2::rc::Retained<CATapDescription>,
) -> Result<AudioObjectID, OSStatus> {
    let mut tap_id: AudioObjectID = 0;

    // Use the objc2_core_audio function with proper signature
    let status = AudioHardwareCreateProcessTap(Some(description.as_ref()), &mut tap_id);

    if status == 0 {
        // kAudioHardwareNoError
        Ok(tap_id)
    } else {
        Err(status)
    }
}

/// Safe wrapper for AudioHardwareDestroyProcessTap
pub unsafe fn destroy_process_tap(tap_id: AudioObjectID) -> Result<(), OSStatus> {
    let status = AudioHardwareDestroyProcessTap(tap_id);

    if status == 0 {
        Ok(())
    } else {
        Err(status)
    }
}

/// Safe wrapper for translating PID to AudioObjectID
/// This uses AudioObjectGetPropertyData instead of AudioHardwareGetProperty
pub unsafe fn translate_pid_to_audio_object(pid: u32) -> Result<AudioObjectID, OSStatus> {
    use std::mem;

    // Use the system object constant from coreaudio-sys
    let system_object = kAudioObjectSystemObject;

    let address = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyTranslatePIDToProcessObject,
        mScope: 0,   // kAudioObjectPropertyScopeGlobal
        mElement: 0, // kAudioObjectPropertyElementMain
    };

    let mut object_id: AudioObjectID = 0;
    let mut data_size = mem::size_of::<AudioObjectID>() as UInt32;

    // The proper API should be AudioObjectGetPropertyData with qualifier
    let status = AudioObjectGetPropertyData(
        system_object,
        &address,
        mem::size_of::<u32>() as UInt32, // qualifier size (PID size)
        &pid as *const u32 as *const c_void, // qualifier data (PID)
        &mut data_size,
        &mut object_id as *mut AudioObjectID as *mut c_void,
    );

    if status == 0 {
        Ok(object_id)
    } else {
        Err(status)
    }
}

/// Convert OSStatus error codes to human-readable messages
pub fn format_osstatus_error(status: OSStatus) -> String {
    match status {
        0 => "No error".to_string(),
        1852797029 => "Audio hardware not running".to_string(),
        2003329396 => "Audio hardware unspecified error".to_string(),
        2003332927 => "Audio hardware unknown property error".to_string(),
        1937010544 => "Audio hardware not running error".to_string(),
        -50 => "Parameter error".to_string(),
        -4 => "Unimplemented error".to_string(),
        _ => format!("Unknown OSStatus error: {}", status),
    }
}

/// Check if Core Audio Taps API is available at runtime
pub fn is_core_audio_taps_available() -> bool {
    // We can't easily check for symbol availability at runtime in Rust
    // Instead, we'll rely on macOS version checking
    // The actual availability check would happen when we try to use the functions

    #[cfg(target_os = "macos")]
    {
        // On macOS, assume available (version check should happen elsewhere)
        true
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tap_description_creation() {
        let desc = CATapDescription::new_for_process(1234);
        assert_eq!(desc.process_array[0], 1234);
        assert_eq!(desc.number_processes, 1);
        assert_eq!(desc.tap_format_type, kAudioTapFormatType_Process);
    }

    #[test]
    fn test_system_tap_description() {
        let desc = CATapDescription::new_for_system();
        assert_eq!(desc.number_processes, 0);
        assert_eq!(desc.tap_format_type, kAudioTapFormatType_Output);
    }

    #[test]
    fn test_error_formatting() {
        assert_eq!(format_osstatus_error(0), "No error");
        assert_ne!(format_osstatus_error(-50), "Unknown OSStatus error: -50");
    }
}
