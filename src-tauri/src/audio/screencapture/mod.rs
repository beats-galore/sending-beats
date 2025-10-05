pub mod discovery;
pub mod ffi;
pub mod stream;

pub use discovery::{get_available_applications, ApplicationInfo};
pub use stream::ScreenCaptureAudioStream;
