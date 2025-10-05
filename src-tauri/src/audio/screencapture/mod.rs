pub mod ffi;
pub mod stream;
pub mod discovery;

pub use stream::ScreenCaptureAudioStream;
pub use discovery::{get_available_applications, ApplicationInfo};
