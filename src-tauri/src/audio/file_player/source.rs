// Turning a file player into an input the mixer can read
//
// Every other input is clocked by hardware: a capture callback hands over a
// buffer every period whether anything is ready or not, and that cadence is what
// paces the pipeline. A file has no such clock — decoding runs as fast as the
// CPU allows, which would fill the queue in an instant and leave the rest as
// drop.
//
// So the queue itself is the clock. This thread tops the ring up to a target and
// then waits, which means audio leaves the file at exactly the rate the mixer
// takes it, and the amount standing in front of the mixer stays bounded.

use colored::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

use super::player::AudioFilePlayer;

/// Frames handed over at a time
///
/// Matches the chunk the input worker is told to expect, so a pass through here
/// lines up with one of its reads rather than straddling two.
pub const SOURCE_CHUNK_FRAMES: usize = 512;

/// How much audio the ring is kept holding, as a multiple of the chunk
///
/// Enough that a late wake-up does not empty it, small enough that it is not
/// heard as delay: three chunks at 48k is about 32ms.
const TARGET_CHUNKS: usize = 3;

/// How long to wait when the ring is full or the player has nothing to say
///
/// A fraction of a chunk, so the ring is topped up promptly once it drains
/// without the thread spinning on a full queue.
const IDLE_POLL: Duration = Duration::from_millis(4);

/// A decoding thread feeding one file player's audio into the pipeline
pub struct FilePlayerSource {
    device_id: String,
    running: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FilePlayerSource {
    /// Start decoding into `producer`
    ///
    /// The thread runs until dropped, whatever the player is doing: pausing
    /// stops audio being produced but leaves the source attached, so the channel
    /// strip it is patched into stays where it is.
    pub fn start(
        device_id: String,
        player: Arc<AudioFilePlayer>,
        mut producer: rtrb::Producer<f32>,
    ) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let thread_running = running.clone();
        let thread_device_id = device_id.clone();

        let (_, channels) = player.output_format();
        let chunk_samples = SOURCE_CHUNK_FRAMES * channels as usize;
        let target_samples = chunk_samples * TARGET_CHUNKS;

        let handle = std::thread::spawn(move || {
            info!(
                "🎼 {}: decoding '{}' into the mixer",
                "FILE_PLAYER_SOURCE".on_magenta().white(),
                thread_device_id
            );

            while thread_running.load(Ordering::Relaxed) {
                // The ring's fill is the clock. Holding back here is what keeps
                // the file playing at its own speed rather than as fast as it
                // decodes.
                let queued = producer.buffer().capacity() - producer.slots();
                if queued >= target_samples || producer.slots() < chunk_samples {
                    std::thread::sleep(IDLE_POLL);
                    continue;
                }

                match player.next_block(SOURCE_CHUNK_FRAMES) {
                    Ok(Some(block)) if block.is_empty() => {
                        // Paused or stopped. Nothing is written, so the mixer
                        // simply finds no audio from this device this cycle.
                        std::thread::sleep(IDLE_POLL);
                    }
                    Ok(Some(block)) => {
                        for sample in block {
                            if producer.push(sample).is_err() {
                                break;
                            }
                        }
                    }
                    Ok(None) => {
                        // The queue played out. The player is left stopped so
                        // the interface reads as finished rather than playing
                        // into silence.
                        player.stop();
                        std::thread::sleep(IDLE_POLL);
                    }
                    Err(e) => {
                        warn!(
                            "⚠️ {}: '{}' stopped decoding: {}",
                            "FILE_PLAYER_SOURCE".on_magenta().white(),
                            thread_device_id,
                            e
                        );
                        player.stop();
                        std::thread::sleep(IDLE_POLL);
                    }
                }
            }

            info!(
                "🛑 {}: stopped decoding '{}'",
                "FILE_PLAYER_SOURCE".on_magenta().white(),
                thread_device_id
            );
        });

        Self {
            device_id,
            running,
            handle: Some(handle),
        }
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }
}

impl Drop for FilePlayerSource {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
