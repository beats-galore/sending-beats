use anyhow::Result;
use colored::Colorize;
use std::ffi::{c_char, c_void, CString};
use std::sync::{Arc, Mutex as StdMutex};
use tracing::{error, info, warn};

use super::ffi;

pub struct ScreenCaptureAudioStream {
    pid: i32,
    stream_ptr: Option<ffi::SCStreamPtr>,
    producer: Option<Arc<StdMutex<rtrb::Producer<f32>>>>,
    app_name: String,
}

impl ScreenCaptureAudioStream {
    pub fn new(pid: i32, app_name: String) -> Self {
        Self {
            pid,
            stream_ptr: None,
            producer: None,
            app_name,
        }
    }

    pub fn start(&mut self, producer: rtrb::Producer<f32>) -> Result<f64> {
        info!(
            "{} {} (PID {})",
            "SC_STREAM_START".blue(),
            self.app_name,
            self.pid
        );

        // Create stream
        let stream_ptr = unsafe { ffi::sc_audio_stream_create(self.pid) };
        if stream_ptr.is_null() {
            anyhow::bail!(
                "Failed to create ScreenCaptureKit stream for PID {}",
                self.pid
            );
        }

        // Store producer in Arc<Mutex> for sharing with callback
        let producer_arc = Arc::new(StdMutex::new(producer));
        self.producer = Some(Arc::clone(&producer_arc));

        // Create context to pass to callbacks
        let context = Box::into_raw(Box::new(StreamContext {
            producer: producer_arc,
            pid: self.pid,
            app_name: self.app_name.clone(),
        })) as *mut c_void;

        // Start capture
        let result = unsafe {
            ffi::sc_audio_stream_start(stream_ptr, audio_sample_callback, error_callback, context)
        };

        if result != 0 {
            // Clean up context on failure
            unsafe {
                let _ = Box::from_raw(context as *mut StreamContext);
            }
            anyhow::bail!(
                "Failed to start ScreenCaptureKit stream: error code {}",
                result
            );
        }

        self.stream_ptr = Some(stream_ptr);

        info!(
            "{} {} (PID {})",
            "SC_STREAM_STARTED".green(),
            self.app_name,
            self.pid
        );

        // Return sample rate (ScreenCaptureKit uses 48kHz)
        Ok(48000.0)
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(stream_ptr) = self.stream_ptr.take() {
            info!(
                "{} {} (PID {})",
                "SC_STREAM_STOP".yellow(),
                self.app_name,
                self.pid
            );

            let result = unsafe { ffi::sc_audio_stream_stop(stream_ptr) };

            if result != 0 {
                warn!(
                    "{} {} (PID {}): error code {}",
                    "SC_STREAM_STOP_ERROR".red(),
                    self.app_name,
                    self.pid,
                    result
                );
            }

            unsafe { ffi::sc_audio_stream_destroy(stream_ptr) };

            info!(
                "{} {} (PID {})",
                "SC_STREAM_STOPPED".green(),
                self.app_name,
                self.pid
            );
        }

        Ok(())
    }
}

impl Drop for ScreenCaptureAudioStream {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

// Context passed to callbacks
struct StreamContext {
    producer: Arc<StdMutex<rtrb::Producer<f32>>>,
    pid: i32,
    app_name: String,
}

// Audio sample callback - called by Swift when audio samples are available
use std::sync::atomic::{AtomicU64, Ordering};

static CALLBACK_COUNTER: AtomicU64 = AtomicU64::new(0);

extern "C" fn audio_sample_callback(
    context: *mut c_void,
    samples: *const f32,
    sample_count: i32,
    channels: i32,
    sample_rate: f64,
) {
    if context.is_null() || samples.is_null() {
        return;
    }

    let count = CALLBACK_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;

    // VERIFY: Log what we receive from Swift
    if count == 1 {
        info!(
            "🎵 {}: First callback - sample_count={}, channels={}, sample_rate={}Hz",
            "SC_AUDIO_CALLBACK".on_purple().cyan(),
            sample_count,
            channels,
            sample_rate
        );
    } else if count % 1000 == 0 {
        info!(
            "🎵 {}: Callback #{} - sample_count={}, channels={}",
            "SC_AUDIO_CALLBACK".on_purple().cyan(),
            count,
            sample_count,
            channels
        );
    }

    let ctx = unsafe { &*(context as *const StreamContext) };

    // Convert raw pointer to slice
    let audio_data = unsafe { std::slice::from_raw_parts(samples, sample_count as usize) };

    // Calculate peak for debugging
    let mut peak = 0.0f32;
    for &sample in audio_data {
        peak = peak.max(sample.abs());
    }

    if count % 1000 == 0 {
        info!(
            "🎵 {}: Peak level from ScreenCaptureKit: {:.4}",
            "SC_AUDIO_PEAK".on_purple().cyan(),
            peak
        );
    }

    // Write to RTRB producer
    if let Ok(mut producer) = ctx.producer.lock() {
        let mut written = 0;
        // Write samples to ringbuffer (drop if full)
        for &sample in audio_data {
            if producer.push(sample).is_ok() {
                written += 1;
            }
        }

        if count % 1000 == 0 {
            info!(
                "🎵 {}: Wrote {}/{} samples to ringbuffer",
                "SC_RTRB_WRITE".on_purple().cyan(),
                written,
                sample_count
            );
        }
    }
}

// Error callback - called by Swift when errors occur
extern "C" fn error_callback(context: *mut c_void, error_message: *const c_char) {
    if context.is_null() {
        error!("{} Context is null", "SC_ERROR".red());
        return;
    }

    let ctx = unsafe { &*(context as *const StreamContext) };

    let error_str = if !error_message.is_null() {
        unsafe { ffi::c_str_to_string(error_message) }
    } else {
        "Unknown error".to_string()
    };

    error!(
        "{} {} (PID {}): {}",
        "SC_ERROR".red(),
        ctx.app_name,
        ctx.pid,
        error_str
    );
}
