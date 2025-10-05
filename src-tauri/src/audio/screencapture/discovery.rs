use anyhow::Result;
use super::ffi;

#[derive(Debug, Clone)]
pub struct ApplicationInfo {
    pub pid: i32,
    pub bundle_identifier: String,
    pub application_name: String,
}

pub fn get_available_applications() -> Result<Vec<ApplicationInfo>> {
    unsafe {
        let mut apps_ptr: *mut ffi::SCAppInfoPtr = std::ptr::null_mut();
        let mut count: i32 = 0;

        let result = ffi::sc_audio_get_available_applications(&mut apps_ptr, &mut count);

        if result != 0 {
            anyhow::bail!("Failed to get available applications: error code {}", result);
        }

        if apps_ptr.is_null() || count == 0 {
            return Ok(Vec::new());
        }

        let mut applications = Vec::with_capacity(count as usize);

        for i in 0..count {
            let app_ptr = *apps_ptr.offset(i as isize) as *const ffi::SCAppInfo;
            if app_ptr.is_null() {
                continue;
            }

            let app = &*app_ptr;

            applications.push(ApplicationInfo {
                pid: app.pid,
                bundle_identifier: ffi::c_str_to_string(app.bundle_identifier),
                application_name: ffi::c_str_to_string(app.application_name),
            });
        }

        ffi::sc_audio_free_applications(apps_ptr, count);

        Ok(applications)
    }
}

pub fn check_screen_recording_permission() -> bool {
    unsafe {
        ffi::sc_audio_check_permission() != 0
    }
}
