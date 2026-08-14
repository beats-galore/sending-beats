pub mod discovery;
pub mod ffi;
pub mod stream;

pub use discovery::{get_available_applications, resolve_application_source, ApplicationInfo};
pub use stream::ScreenCaptureAudioStream;
