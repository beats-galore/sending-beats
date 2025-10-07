use super::ffi;
use anyhow::Result;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub struct ApplicationInfo {
    pub pid: i32,
    pub bundle_identifier: String,
    pub application_name: String,
}

pub fn get_available_applications() -> Result<Vec<ApplicationInfo>> {
    info!("📺 Calling ScreenCaptureKit to get available applications...");

    unsafe {
        let mut apps_ptr: *mut ffi::SCAppInfoPtr = std::ptr::null_mut();
        let mut count: i32 = 0;

        let result = ffi::sc_audio_get_available_applications(&mut apps_ptr, &mut count);

        if result != 0 {
            let error_msg = match result {
                -1 => "ScreenCaptureKit error (likely permission denied or API failure)",
                -2 => "ScreenCaptureKit timeout (took longer than 10 seconds)",
                code => &format!("Unknown error code: {}", code),
            };
            error!("❌ ScreenCaptureKit failed: {}", error_msg);
            anyhow::bail!(
                "Failed to get available applications: {} (error code {})",
                error_msg,
                result
            );
        }

        if apps_ptr.is_null() || count == 0 {
            warn!("⚠️ ScreenCaptureKit returned no applications");
            return Ok(Vec::new());
        }

        info!("✅ ScreenCaptureKit returned {} applications", count);

        let mut applications = Vec::with_capacity(count as usize);

        for i in 0..count {
            let app_ptr = *apps_ptr.offset(i as isize);
            if app_ptr.is_null() {
                continue;
            }

            let pid = ffi::sc_audio_app_get_pid(app_ptr);
            let bundle_id = ffi::c_str_to_string(ffi::sc_audio_app_get_bundle_id(app_ptr));
            let app_name = ffi::c_str_to_string(ffi::sc_audio_app_get_name(app_ptr));

            applications.push(ApplicationInfo {
                pid,
                bundle_identifier: bundle_id,
                application_name: app_name,
            });
        }

        ffi::sc_audio_free_applications(apps_ptr, count);

        info!("📋 Parsed {} application entries", applications.len());
        Ok(applications)
    }
}

pub fn check_screen_recording_permission() -> bool {
    unsafe { ffi::sc_audio_check_permission() != 0 }
}
