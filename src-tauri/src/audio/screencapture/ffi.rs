use std::ffi::{CStr, c_char, c_void};
use std::os::raw::c_int;

// C-compatible callback types matching Swift side
pub type AudioSampleCallback = extern "C" fn(
    context: *mut c_void,
    samples: *const f32,
    sample_count: i32,
    channels: i32,
    sample_rate: f64,
);

pub type ErrorCallback = extern "C" fn(
    context: *mut c_void,
    error_message: *const c_char,
);

// Opaque pointer types for Swift objects
pub type SCAppInfoPtr = *mut c_void;
pub type SCStreamPtr = *mut c_void;

#[repr(C)]
pub struct SCAppInfo {
    pub pid: i32,
    pub bundle_identifier: *const c_char,
    pub application_name: *const c_char,
}

// External Swift functions
extern "C" {
    pub fn sc_audio_get_available_applications(
        out_apps: *mut *mut SCAppInfoPtr,
        out_count: *mut i32,
    ) -> i32;

    pub fn sc_audio_free_applications(
        apps: *mut SCAppInfoPtr,
        count: i32,
    );

    pub fn sc_audio_stream_create(pid: i32) -> SCStreamPtr;

    pub fn sc_audio_stream_start(
        stream: SCStreamPtr,
        audio_callback: AudioSampleCallback,
        error_callback: ErrorCallback,
        context: *mut c_void,
    ) -> i32;

    pub fn sc_audio_stream_stop(stream: SCStreamPtr) -> i32;

    pub fn sc_audio_stream_destroy(stream: SCStreamPtr);

    pub fn sc_audio_check_permission() -> i32;
}

// Helper to convert C string to Rust String
pub unsafe fn c_str_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    CStr::from_ptr(ptr)
        .to_string_lossy()
        .into_owned()
}
