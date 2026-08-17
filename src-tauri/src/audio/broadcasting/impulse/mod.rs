// Impulse — broadcasting by sending the mix as bounded segments.
//
// Named for the worker that answers, the same way Icecast is named for the
// server that answers it. It is not a variant of Icecast with different fields:
// nothing is held open, there is no source connection, and the unit of a
// broadcast is a few seconds of audio in its own request rather than a stream of
// bytes down a socket.
//
// That shape is forced rather than chosen. Cloudflare buffers a streaming
// request body and only invokes the worker once the request has completed, so a
// show sent as one long connection materialises after it has ended. Segmenting
// has to happen before the edge, and this is the thing that does it.
//
// - mp3_frames: where a frame starts and ends, and how long it lasts
// - packed_audio_id3: the tag without which a player will not advance
// - segmenter: whole frames, gathered until there are enough for a segment
// - uploader: one bounded request per segment
// - send_loop: the mix in, segments out
// - service: going on air, coming off it, and saying which

pub mod mp3_frames;
pub mod packed_audio_id3;
pub mod segmenter;
pub mod send_loop;
pub mod service;
pub mod uploader;

pub use segmenter::{Mp3Segmenter, Segment};
pub use send_loop::{ImpulseSendLoop, ImpulseStats};
pub use service::{get_impulse_service, ImpulseConfig, ImpulseService};
pub use uploader::{ImpulseUploader, LiveStatus, SegmentMetadata};
