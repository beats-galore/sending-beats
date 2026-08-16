// Taking the mix off the ring buffer, encoding it, and putting it on the wire.
//
// This is the step that was missing. Everything either side of it existed: the
// pipeline hands over an RTRB consumer of the mix, and `IcecastSourceClient`
// can open a SOURCE connection and write bytes to it. In between was a task
// that read samples, counted them, and dropped them on the floor behind a
// `TODO` — so going on air opened no connection and sent no audio.
//
// It also spun. With nothing to send it slept 100 microseconds and looked
// again, forever, with no way to stop it: each time you went live left another
// one running.

use anyhow::Result;
use colored::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use super::icecast_source::IcecastSourceClient;
use crate::audio::recording::lame::Lame;

/// Samples taken from the ring before encoding
///
/// A quarter of a second at 48k stereo. Large enough that the encoder is given
/// whole frames to work with rather than being called on a handful of samples,
/// small enough that the delay it adds is not heard as latency on the stream.
const BATCH_SAMPLES: usize = 24_000;

/// How long to wait when the ring is empty
///
/// The mixer produces in blocks, so an empty ring means the next block has not
/// landed yet. Roughly one block at 48k — the previous loop waited 100
/// microseconds, which is a busy-wait dressed as a sleep and cost a core.
const IDLE_WAIT: Duration = Duration::from_millis(5);

/// How much silence to send when the mix stops arriving
///
/// Icecast drops a source that stops writing. A gap in the audio is better than
/// being disconnected, so a stall is filled rather than left.
const STALL_AFTER: Duration = Duration::from_millis(500);

/// A running broadcast: the task, and the switch that ends it
#[derive(Debug)]
pub struct SendLoop {
    shutdown: Arc<AtomicBool>,
    handle: tokio::task::JoinHandle<()>,
    /// Shared with the task so the interface can report what has gone out
    stats: Arc<Mutex<SendStats>>,
}

/// What the broadcast has done so far
#[derive(Debug, Clone, Default)]
pub struct SendStats {
    pub samples_read: u64,
    pub bytes_sent: u64,
    pub encode_errors: u64,
    pub send_errors: u64,
}

impl SendLoop {
    /// Start reading the mix, encoding it, and sending it
    ///
    /// The client must already be connected: failing to reach the server is
    /// something the caller has to be told about, not something to discover
    /// inside a task nobody is waiting on.
    pub fn start(
        client: IcecastSourceClient,
        consumer: rtrb::Consumer<f32>,
        sample_rate: u32,
        channels: u16,
        kilobitrate: u32,
    ) -> Result<Self> {
        let encoder = Lame::new(sample_rate, channels, kilobitrate)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(Mutex::new(SendStats::default()));

        let handle = tokio::spawn(run(
            client,
            consumer,
            encoder,
            channels,
            Arc::clone(&shutdown),
            Arc::clone(&stats),
        ));

        Ok(Self {
            shutdown,
            handle,
            stats,
        })
    }

    /// Stop sending and wait for the connection to close
    ///
    /// Waited on rather than aborted: the encoder has frames still inside it
    /// and the server is owed a clean disconnect, neither of which happens if
    /// the task is killed where it stands.
    pub async fn stop(self) {
        self.shutdown.store(true, Ordering::Relaxed);

        if let Err(e) = self.handle.await {
            warn!(
                "⚠️ {}: The send loop did not finish cleanly: {}",
                "ICECAST_SEND".on_blue().white(),
                e
            );
        }
    }

    pub async fn stats(&self) -> SendStats {
        self.stats.lock().await.clone()
    }
}

/// Take up to `BATCH_SAMPLES` off the ring
fn drain(consumer: &mut rtrb::Consumer<f32>, into: &mut Vec<f32>) {
    into.clear();

    while into.len() < BATCH_SAMPLES {
        match consumer.pop() {
            Ok(sample) => into.push(sample),
            Err(_) => break,
        }
    }
}

async fn run(
    mut client: IcecastSourceClient,
    mut consumer: rtrb::Consumer<f32>,
    mut encoder: Lame,
    channels: u16,
    shutdown: Arc<AtomicBool>,
    stats: Arc<Mutex<SendStats>>,
) {
    info!(
        "🎙️ {}: Sending the mix to Icecast",
        "ICECAST_SEND".on_blue().white()
    );

    let mut batch: Vec<f32> = Vec::with_capacity(BATCH_SAMPLES);
    let mut last_audio = std::time::Instant::now();

    while !shutdown.load(Ordering::Relaxed) {
        drain(&mut consumer, &mut batch);

        if batch.is_empty() {
            // Icecast drops a source that goes quiet, so a stall is filled with
            // silence rather than left as a gap in the connection.
            if last_audio.elapsed() >= STALL_AFTER {
                let quiet = vec![0.0_f32; BATCH_SAMPLES.min(4096) * channels.max(1) as usize];
                send_encoded(&mut client, &mut encoder, &quiet, &stats).await;
                last_audio = std::time::Instant::now();
            }

            tokio::time::sleep(IDLE_WAIT).await;
            continue;
        }

        last_audio = std::time::Instant::now();
        stats.lock().await.samples_read += batch.len() as u64;

        if !send_encoded(&mut client, &mut encoder, &batch, &stats).await {
            break;
        }
    }

    // The encoder still holds frames, and the server is owed a goodbye.
    match encoder.flush() {
        Ok(tail) if !tail.is_empty() => {
            if let Err(e) = client.send_audio_data(&tail).await {
                warn!(
                    "⚠️ {}: Could not send the final frames: {}",
                    "ICECAST_SEND".on_blue().white(),
                    e
                );
            }
        }
        Ok(_) => {}
        Err(e) => warn!(
            "⚠️ {}: Could not flush the encoder: {}",
            "ICECAST_SEND".on_blue().white(),
            e
        ),
    }

    if let Err(e) = client.disconnect().await {
        warn!(
            "⚠️ {}: Could not close the connection cleanly: {}",
            "ICECAST_SEND".on_blue().white(),
            e
        );
    }

    info!(
        "🛑 {}: Stopped sending to Icecast",
        "ICECAST_SEND".on_blue().white()
    );
}

/// Encode one batch and put it on the wire. False means the connection is gone.
async fn send_encoded(
    client: &mut IcecastSourceClient,
    encoder: &mut Lame,
    samples: &[f32],
    stats: &Arc<Mutex<SendStats>>,
) -> bool {
    let encoded = match encoder.encode(samples) {
        Ok(bytes) => bytes,
        Err(e) => {
            error!(
                "❌ {}: Encoding failed: {}",
                "ICECAST_SEND".on_blue().white(),
                e
            );
            stats.lock().await.encode_errors += 1;
            // One bad batch is not a reason to leave the air.
            return true;
        }
    };

    if encoded.is_empty() {
        return true;
    }

    match client.send_audio_data(&encoded).await {
        Ok(()) => {
            stats.lock().await.bytes_sent += encoded.len() as u64;
            true
        }
        Err(e) => {
            // The socket is the broadcast. Losing it is the end of this
            // session, and reconnecting is the caller's decision to make.
            error!(
                "❌ {}: Lost the connection to Icecast: {}",
                "ICECAST_SEND".on_blue().white(),
                e
            );
            stats.lock().await.send_errors += 1;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::broadcasting::icecast_source::{AudioCodec, AudioFormat};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn draining_takes_what_is_there_and_stops() {
        let (mut producer, consumer) = rtrb::RingBuffer::<f32>::new(64);
        for n in 0..10 {
            producer.push(n as f32).unwrap();
        }

        let mut consumer = consumer;
        let mut batch = Vec::new();
        drain(&mut consumer, &mut batch);

        assert_eq!(batch.len(), 10, "everything available, and no spinning");
    }

    #[test]
    fn draining_an_empty_ring_yields_nothing() {
        let (_producer, consumer) = rtrb::RingBuffer::<f32>::new(16);

        let mut consumer = consumer;
        let mut batch = vec![1.0, 2.0];
        drain(&mut consumer, &mut batch);

        assert!(batch.is_empty(), "the batch is cleared before it is filled");
    }

    #[test]
    fn draining_stops_at_the_batch_size() {
        let (mut producer, consumer) = rtrb::RingBuffer::<f32>::new(BATCH_SAMPLES * 2);
        for _ in 0..BATCH_SAMPLES + 500 {
            producer.push(0.5).unwrap();
        }

        let mut consumer = consumer;
        let mut batch = Vec::new();
        drain(&mut consumer, &mut batch);

        assert_eq!(batch.len(), BATCH_SAMPLES, "one batch at a time");
    }
}

/// End-to-end against a stand-in Icecast server.
///
/// The bug this covers is that nothing was ever sent: the loop read the ring,
/// counted the samples and dropped them. Counting samples inside the loop
/// cannot tell you that, so these tests stand up a socket that speaks enough of
/// the SOURCE protocol to accept a source, and check what actually arrives on
/// it.
#[cfg(test)]
mod wire_tests {
    use super::*;
    use crate::audio::broadcasting::icecast_source::{
        AudioCodec, AudioFormat, IcecastSourceClient,
    };
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::Mutex as AsyncMutex;

    struct Received {
        request: String,
        body: Vec<u8>,
    }

    /// A socket that accepts one source and records everything it is sent
    async fn fake_icecast() -> (u16, Arc<AsyncMutex<Received>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let received = Arc::new(AsyncMutex::new(Received {
            request: String::new(),
            body: Vec::new(),
        }));
        let recording = Arc::clone(&received);

        tokio::spawn(async move {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };

            let mut head = vec![0u8; 2048];
            let Ok(read) = socket.read(&mut head).await else {
                return;
            };
            recording.lock().await.request = String::from_utf8_lossy(&head[..read]).into_owned();

            if socket.write_all(b"HTTP/1.0 200 OK\r\n\r\n").await.is_err() {
                return;
            }

            let mut chunk = vec![0u8; 8192];
            while let Ok(read) = socket.read(&mut chunk).await {
                if read == 0 {
                    break;
                }
                recording
                    .lock()
                    .await
                    .body
                    .extend_from_slice(&chunk[..read]);
            }
        });

        (port, received)
    }

    fn format() -> AudioFormat {
        AudioFormat {
            sample_rate: 48_000,
            channels: 2,
            bitrate: 128,
            codec: AudioCodec::Mp3,
        }
    }

    #[tokio::test]
    async fn going_on_air_connects_and_sends_encoded_audio() {
        let (port, received) = fake_icecast().await;

        let mut client = IcecastSourceClient::new(
            "127.0.0.1".to_string(),
            port,
            "/live".to_string(),
            "hunter2".to_string(),
            format(),
        );

        client.connect().await.expect("the server accepts a source");

        let (mut producer, consumer) = rtrb::RingBuffer::<f32>::new(BATCH_SAMPLES * 4);
        for n in 0..BATCH_SAMPLES * 2 {
            producer.push(((n as f32) * 0.01).sin() * 0.4).unwrap();
        }

        let sending = SendLoop::start(client, consumer, 48_000, 2, 128).expect("starts");

        // Long enough for a couple of batches to be drained and encoded.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let stats = sending.stats().await;
        sending.stop().await;

        let seen = received.lock().await;

        assert!(
            seen.request.starts_with("SOURCE /live"),
            "a SOURCE request was made: {}",
            seen.request.lines().next().unwrap_or("")
        );
        assert!(
            seen.request.contains("Authorization: Basic "),
            "the request carries credentials"
        );
        assert!(
            seen.request.contains("audio/mpeg"),
            "the request declares MP3"
        );

        assert!(stats.samples_read > 0, "the mix was read off the ring");
        assert!(
            stats.bytes_sent > 0,
            "encoded audio was written to the socket"
        );
        assert_eq!(stats.send_errors, 0, "nothing failed on the way out");

        assert!(!seen.body.is_empty(), "the server actually received audio");
        assert_eq!(
            seen.body[0], 0xFF,
            "what arrived is MP3, not raw PCM relabelled"
        );
        assert_eq!(seen.body[1] & 0xE0, 0xE0, "the MPEG sync word is intact");
    }

    /// Coming off air used to leave the task spinning forever with no way to
    /// stop it, so every toggle leaked another one.
    #[tokio::test]
    async fn coming_off_air_ends_the_task() {
        let (port, _received) = fake_icecast().await;

        let mut client = IcecastSourceClient::new(
            "127.0.0.1".to_string(),
            port,
            "/live".to_string(),
            "hunter2".to_string(),
            format(),
        );
        client.connect().await.expect("connects");

        let (_producer, consumer) = rtrb::RingBuffer::<f32>::new(1024);
        let sending = SendLoop::start(client, consumer, 48_000, 2, 128).expect("starts");

        // Returns rather than hanging, which is the whole assertion.
        tokio::time::timeout(Duration::from_secs(5), sending.stop())
            .await
            .expect("the send loop stops when asked");
    }

    #[tokio::test]
    async fn a_refused_source_is_reported_rather_than_swallowed() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut head = vec![0u8; 1024];
                let _ = socket.read(&mut head).await;
                let _ = socket.write_all(b"HTTP/1.0 401 Unauthorized\r\n\r\n").await;
            }
        });

        let mut client = IcecastSourceClient::new(
            "127.0.0.1".to_string(),
            port,
            "/live".to_string(),
            "wrong".to_string(),
            format(),
        );

        let error = client.connect().await.expect_err("a refusal is an error");
        assert!(
            error.to_string().contains("Authentication"),
            "the reason is carried through: {}",
            error
        );
    }
}
