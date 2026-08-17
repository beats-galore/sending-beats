// Taking the mix off the ring buffer, cutting it into segments, and sending each
// one as its own request.
//
// Two tasks, not one. Encoding runs on the mixer's schedule and must never wait
// for the network; uploading runs on the network's schedule and must never hold
// up encoding. A queue between them absorbs the difference, and when it cannot,
// a segment is dropped and the next one is flagged as following a gap — because
// a broadcast that stalls behind a slow request is worse than one with a seam in
// it.

use anyhow::Result;
use colored::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

use super::segmenter::{Mp3Segmenter, Segment};
use super::uploader::{ImpulseUploader, SegmentMetadata};
use crate::audio::broadcasting::metadata;
use crate::audio::recording::lame::Lame;

/// Samples taken from the ring before encoding
///
/// A quarter of a second at 48k stereo — the same batch the Icecast loop uses,
/// and for the same reason: whole frames for the encoder to work on, without
/// adding a delay anyone would hear.
const BATCH_SAMPLES: usize = 24_000;

/// How long to wait when the ring is empty
const IDLE_WAIT: Duration = Duration::from_millis(5);

/// How long the mix can stop arriving before silence is put in its place
///
/// The timeline has to stay continuous. A gap left as a gap becomes a segment
/// whose duration does not match the time it covers, and the playlist walks away
/// from the clock.
const STALL_AFTER: Duration = Duration::from_millis(500);

/// Segments allowed to wait for the network
///
/// At four seconds each this is half a minute of audio. Past that the uploader
/// is not behind, it is failing, and holding more would only mean sending audio
/// that has already fallen out of the listener's window.
const QUEUE_DEPTH: usize = 8;

/// A running broadcast: the two tasks, and the switch that ends them
#[derive(Debug)]
pub struct ImpulseSendLoop {
    shutdown: Arc<AtomicBool>,
    encoding: tokio::task::JoinHandle<()>,
    uploading: tokio::task::JoinHandle<()>,
    stats: Arc<Mutex<ImpulseStats>>,
}

/// What the broadcast has done so far
#[derive(Debug, Clone, Default)]
pub struct ImpulseStats {
    pub samples_read: u64,
    pub segments_sent: u64,
    pub bytes_sent: u64,
    pub segments_dropped: u64,
    pub encode_errors: u64,
    pub send_errors: u64,
    /// What the far end last said about the station
    pub on_air: bool,
    pub media_sequence: u64,
    pub last_error: Option<String>,
}

/// Everything the two tasks share
#[derive(Debug)]
struct Shared {
    stats: Arc<Mutex<ImpulseStats>>,
    /// Set whenever a segment does not land, and consumed by the next one that
    /// does. Whether audio was interrupted is knowable only at the moment it
    /// happens, so it is recorded then rather than inferred later.
    gap: Arc<AtomicBool>,
}

impl ImpulseSendLoop {
    /// Start reading the mix, segmenting it, and sending it
    pub fn start(
        uploader: ImpulseUploader,
        consumer: rtrb::Consumer<f32>,
        sample_rate: u32,
        channels: u16,
        kilobitrate: u32,
        segment_ms: u64,
    ) -> Result<Self> {
        // Segmented rather than whole-file: the seek table describes a length
        // this has no end for, and a frame that borrows bits from the one before
        // it cannot be cut away from it.
        let encoder = Lame::for_segments(sample_rate, channels, kilobitrate)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(Mutex::new(ImpulseStats::default()));
        let shared = Shared {
            stats: Arc::clone(&stats),
            gap: Arc::new(AtomicBool::new(false)),
        };

        let (segments_tx, segments_rx) = mpsc::channel::<Segment>(QUEUE_DEPTH);

        let encoding = tokio::spawn(encode(
            consumer,
            encoder,
            Mp3Segmenter::new(segment_ms),
            channels,
            segments_tx,
            Arc::clone(&shutdown),
            Shared {
                stats: Arc::clone(&shared.stats),
                gap: Arc::clone(&shared.gap),
            },
        ));

        let uploading = tokio::spawn(upload(uploader, segments_rx, shared));

        Ok(Self {
            shutdown,
            encoding,
            uploading,
            stats,
        })
    }

    /// Stop sending, finish the queue, and sign off
    ///
    /// Waited on rather than aborted. The encoder still holds frames, the last
    /// segment is still worth sending, and the station is owed a sign-off — none
    /// of which happens if the tasks are killed where they stand.
    pub async fn stop(self) {
        self.shutdown.store(true, Ordering::Relaxed);

        if let Err(e) = self.encoding.await {
            warn!(
                "⚠️ {}: The encoder did not finish cleanly: {}",
                "IMPULSE_SEND".on_purple().white(),
                e
            );
        }

        if let Err(e) = self.uploading.await {
            warn!(
                "⚠️ {}: The uploader did not finish cleanly: {}",
                "IMPULSE_SEND".on_purple().white(),
                e
            );
        }
    }

    pub async fn stats(&self) -> ImpulseStats {
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

/// Read the mix, encode it, and hand whole segments to the uploader
async fn encode(
    mut consumer: rtrb::Consumer<f32>,
    mut encoder: Lame,
    mut segmenter: Mp3Segmenter,
    channels: u16,
    segments: mpsc::Sender<Segment>,
    shutdown: Arc<AtomicBool>,
    shared: Shared,
) {
    info!(
        "🎙️ {}: Cutting the mix into segments",
        "IMPULSE_SEND".on_purple().white()
    );

    let mut batch: Vec<f32> = Vec::with_capacity(BATCH_SAMPLES);
    let mut last_audio = std::time::Instant::now();

    while !shutdown.load(Ordering::Relaxed) {
        drain(&mut consumer, &mut batch);

        if batch.is_empty() {
            if last_audio.elapsed() >= STALL_AFTER {
                let quiet = vec![0.0_f32; 4_096 * channels.max(1) as usize];
                feed(&mut encoder, &mut segmenter, &quiet, &shared).await;
                last_audio = std::time::Instant::now();
            }

            offer(&mut segmenter, &segments, &shared).await;
            tokio::time::sleep(IDLE_WAIT).await;
            continue;
        }

        last_audio = std::time::Instant::now();
        shared.stats.lock().await.samples_read += batch.len() as u64;

        feed(&mut encoder, &mut segmenter, &batch, &shared).await;
        offer(&mut segmenter, &segments, &shared).await;
    }

    // The encoder still holds frames, and the last few seconds of a show are
    // still the show.
    match encoder.flush() {
        Ok(tail) => segmenter.push(&tail),
        Err(e) => warn!(
            "⚠️ {}: Could not flush the encoder: {}",
            "IMPULSE_SEND".on_purple().white(),
            e
        ),
    }

    offer(&mut segmenter, &segments, &shared).await;

    if let Some(last) = segmenter.flush() {
        // Blocking here is correct: nothing is producing audio any more, and the
        // final segment is worth waiting for room to send.
        if segments.send(last).await.is_err() {
            warn!(
                "⚠️ {}: The final segment had nowhere to go",
                "IMPULSE_SEND".on_purple().white()
            );
        }
    }

    info!(
        "🛑 {}: Stopped cutting segments",
        "IMPULSE_SEND".on_purple().white()
    );
}

/// Encode one batch into the segmenter
async fn feed(encoder: &mut Lame, segmenter: &mut Mp3Segmenter, samples: &[f32], shared: &Shared) {
    match encoder.encode(samples) {
        Ok(encoded) => segmenter.push(&encoded),
        Err(e) => {
            error!(
                "❌ {}: Encoding failed: {}",
                "IMPULSE_SEND".on_purple().white(),
                e
            );
            shared.stats.lock().await.encode_errors += 1;
            // One bad batch is a hole in the audio, not a reason to leave the
            // air — but the next segment does not continue the last one.
            shared.gap.store(true, Ordering::Relaxed);
        }
    }
}

/// Hand over every segment that is ready, dropping any that will not fit
async fn offer(segmenter: &mut Mp3Segmenter, segments: &mpsc::Sender<Segment>, shared: &Shared) {
    while let Some(segment) = segmenter.take() {
        if segments.try_send(segment).is_err() {
            // The uploader is not keeping up. Waiting would stall encoding and
            // put the whole broadcast behind the network.
            warn!(
                "⚠️ {}: Dropped a segment — the uploader is behind",
                "IMPULSE_SEND".on_purple().white()
            );
            shared.stats.lock().await.segments_dropped += 1;
            shared.gap.store(true, Ordering::Relaxed);
        }
    }
}

/// Send each segment as its own request, in order
async fn upload(uploader: ImpulseUploader, mut segments: mpsc::Receiver<Segment>, shared: Shared) {
    while let Some(segment) = segments.recv().await {
        let metadata = describe(&shared);

        match uploader.put_segment(&segment, &metadata).await {
            Ok(status) => {
                let mut stats = shared.stats.lock().await;
                stats.segments_sent += 1;
                stats.bytes_sent += segment.body.len() as u64;
                stats.on_air = status.on_air;
                stats.media_sequence = status.media_sequence;
                stats.last_error = None;
            }
            Err(e) => {
                error!(
                    "❌ {}: A segment did not land: {}",
                    "IMPULSE_SEND".on_purple().white(),
                    e
                );

                let mut stats = shared.stats.lock().await;
                stats.send_errors += 1;
                stats.last_error = Some(e.to_string());
                drop(stats);

                // The audio in this segment is gone, so whatever follows does
                // not continue it.
                shared.gap.store(true, Ordering::Relaxed);
            }
        }
    }

    if let Err(e) = uploader.go_off_air().await {
        warn!(
            "⚠️ {}: Could not sign off: {}",
            "IMPULSE_SEND".on_purple().white(),
            e
        );
    }

    shared.stats.lock().await.on_air = false;

    info!("🛑 {}: Off air", "IMPULSE_SEND".on_purple().white());
}

/// What to say about the segment being sent
///
/// The gap flag is taken rather than read: it describes one seam, and leaving it
/// set would mark every remaining segment as following a gap.
fn describe(shared: &Shared) -> SegmentMetadata {
    let track = metadata::current();

    SegmentMetadata {
        title: track.as_ref().map(|t| t.title.clone()),
        artist: track.as_ref().map(|t| t.artist.clone()),
        discontinuity: shared.gap.swap(false, Ordering::Relaxed),
    }
}

/// End to end against a stand-in ingest worker.
///
/// The thing worth proving is not that the parts work — they each have their own
/// tests — but that what comes out of the socket is a segment: bounded, framed,
/// tagged, and described by a duration that matches the audio inside it. None of
/// the unit tests can see that, because none of them watch the wire.
#[cfg(test)]
mod wire_tests {
    use super::*;
    use crate::audio::broadcasting::impulse::uploader::ImpulseUploader;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::Mutex as AsyncMutex;

    /// One request as the far end saw it
    #[derive(Debug, Clone)]
    struct Request {
        line: String,
        body: Vec<u8>,
    }

    const REPLY: &str =
        r#"{"stationSlug":"shady","onAir":true,"mediaSequence":3,"segmentSeconds":4}"#;

    /// A socket that answers the ingest routes and records what it was sent
    async fn fake_impulse() -> (u16, Arc<AsyncMutex<Vec<Request>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let received = Arc::new(AsyncMutex::new(Vec::<Request>::new()));
        let recording = Arc::clone(&received);

        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let recording = Arc::clone(&recording);
                tokio::spawn(async move {
                    serve(socket, recording).await;
                });
            }
        });

        (port, received)
    }

    /// Read requests off one connection until it closes
    ///
    /// Written out rather than reached for from a crate because the point is to
    /// see the bytes: a server that parsed them for us could hide exactly the
    /// framing this is here to check.
    async fn serve(mut socket: tokio::net::TcpStream, recording: Arc<AsyncMutex<Vec<Request>>>) {
        let mut buffer: Vec<u8> = Vec::new();

        loop {
            let head_end = loop {
                if let Some(at) = find(&buffer, b"\r\n\r\n") {
                    break at;
                }

                let mut chunk = [0u8; 4096];
                match socket.read(&mut chunk).await {
                    Ok(0) | Err(_) => return,
                    Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                }
            };

            let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
            let length = content_length(&head);
            let body_at = head_end + 4;

            while buffer.len() < body_at + length {
                let mut chunk = [0u8; 4096];
                match socket.read(&mut chunk).await {
                    Ok(0) | Err(_) => return,
                    Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                }
            }

            recording.lock().await.push(Request {
                line: head.lines().next().unwrap_or("").to_string(),
                body: buffer[body_at..body_at + length].to_vec(),
            });

            let response = format!(
                "HTTP/1.1 201 Created\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                REPLY.len(),
                REPLY
            );

            if socket.write_all(response.as_bytes()).await.is_err() {
                return;
            }

            buffer.drain(..body_at + length);
        }
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn content_length(head: &str) -> usize {
        head.lines()
            .find(|line| line.to_ascii_lowercase().starts_with("content-length:"))
            .and_then(|line| line.split(':').nth(1))
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0)
    }

    /// A second of a tone, which is two 500 ms segments plus change
    fn one_second_of_audio() -> rtrb::Consumer<f32> {
        let samples = 48_000 * 2;
        let (mut producer, consumer) = rtrb::RingBuffer::<f32>::new(samples * 2);

        for n in 0..samples {
            producer.push(((n as f32) * 0.01).sin() * 0.4).unwrap();
        }

        consumer
    }

    async fn broadcast_one_second(port: u16) -> ImpulseStats {
        let uploader = ImpulseUploader::new(&format!("http://127.0.0.1:{}", port), "shady", "tkn")
            .expect("an uploader");

        let sending = ImpulseSendLoop::start(uploader, one_second_of_audio(), 48_000, 2, 128, 500)
            .expect("the broadcast starts");

        // Long enough for the ring to be drained and the segments to be sent;
        // `stop` then flushes whatever is left and waits for the queue.
        tokio::time::sleep(Duration::from_millis(300)).await;

        let stats = sending.stats().await;
        sending.stop().await;

        stats
    }

    #[tokio::test]
    async fn going_on_air_sends_segments_as_bounded_requests() {
        let (port, received) = fake_impulse().await;
        broadcast_one_second(port).await;

        let requests = received.lock().await.clone();
        let segments: Vec<&Request> = requests
            .iter()
            .filter(|request| request.line.starts_with("PUT /ingest/shady/segment"))
            .collect();

        assert!(
            !segments.is_empty(),
            "segments were sent: {:?}",
            requests.iter().map(|r| &r.line).collect::<Vec<_>>()
        );

        for segment in &segments {
            assert!(
                segment.line.contains("durationMs="),
                "every segment states its own measured duration: {}",
                segment.line
            );
            assert!(
                segment.line.contains("extension=mp3"),
                "and what it carries: {}",
                segment.line
            );
            assert!(
                !segment.body.is_empty(),
                "a segment with a body, not an empty request"
            );
        }
    }

    /// The failure this guards is silent: a segment with no tag fetches fine,
    /// parses fine, and then the player never advances
    #[tokio::test]
    async fn every_segment_opens_with_the_id3_timestamp() {
        let (port, received) = fake_impulse().await;
        broadcast_one_second(port).await;

        let requests = received.lock().await.clone();
        let segments: Vec<&Request> = requests
            .iter()
            .filter(|request| request.line.starts_with("PUT /ingest/shady/segment"))
            .collect();

        assert!(!segments.is_empty(), "there is something to check");

        for segment in segments {
            assert_eq!(&segment.body[0..3], b"ID3", "the tag opens the segment");
            assert!(
                find(
                    &segment.body,
                    b"com.apple.streaming.transportStreamTimestamp"
                )
                .is_some(),
                "and it is the PRIV tag the spec names"
            );
        }
    }

    /// What arrives has to be MP3 the far end can hand to a decoder, not PCM
    /// with a content type bolted on
    #[tokio::test]
    async fn what_arrives_is_mp3_after_the_tag() {
        let (port, received) = fake_impulse().await;
        broadcast_one_second(port).await;

        let requests = received.lock().await.clone();
        let segment = requests
            .iter()
            .find(|request| request.line.starts_with("PUT /ingest/shady/segment"))
            .expect("a segment");

        let tag_size = u32::from_be_bytes([
            segment.body[6],
            segment.body[7],
            segment.body[8],
            segment.body[9],
        ]);
        let declared = (tag_size & 0x7F)
            | ((tag_size >> 8) & 0x7F) << 7
            | ((tag_size >> 16) & 0x7F) << 14
            | ((tag_size >> 24) & 0x7F) << 21;

        let audio = &segment.body[10 + declared as usize..];

        assert_eq!(audio[0], 0xFF, "the MPEG sync word follows the tag");
        assert_eq!(audio[1] & 0xE0, 0xE0, "and it is intact");
    }

    /// Without a sign-off the station stays on air until the dead-air alarm
    /// notices, so listeners get the end of the show and then a stall
    #[tokio::test]
    async fn coming_off_air_signs_off() {
        let (port, received) = fake_impulse().await;
        broadcast_one_second(port).await;

        let requests = received.lock().await.clone();
        let control = requests
            .iter()
            .find(|request| request.line.starts_with("POST /ingest/shady/control"))
            .expect("the broadcast signed off");

        assert!(
            String::from_utf8_lossy(&control.body).contains("off-air"),
            "and said so explicitly"
        );
        assert_eq!(
            requests.last().map(|request| request.line.as_str()),
            Some(control.line.as_str()),
            "signing off is the last thing sent, after the final segment"
        );
    }

    #[tokio::test]
    async fn the_far_ends_answer_is_what_is_reported() {
        let (port, _received) = fake_impulse().await;
        let stats = broadcast_one_second(port).await;

        assert!(stats.segments_sent > 0, "segments landed");
        assert!(stats.on_air, "the far end said the station is on air");
        assert_eq!(stats.media_sequence, 3, "as read from its reply");
        assert_eq!(stats.send_errors, 0, "nothing failed on the way out");
        assert_eq!(stats.segments_dropped, 0, "and nothing was dropped");
    }

    /// Coming off air used to be the case that leaked a task. It has to return
    /// rather than hang, which is the whole assertion.
    #[tokio::test]
    async fn stopping_finishes_rather_than_hanging() {
        let (port, _received) = fake_impulse().await;

        let uploader = ImpulseUploader::new(&format!("http://127.0.0.1:{}", port), "shady", "tkn")
            .expect("an uploader");
        let (_producer, consumer) = rtrb::RingBuffer::<f32>::new(1024);

        let sending =
            ImpulseSendLoop::start(uploader, consumer, 48_000, 2, 128, 4_000).expect("starts");

        tokio::time::timeout(Duration::from_secs(10), sending.stop())
            .await
            .expect("the broadcast stops when asked");
    }
}

#[cfg(test)]
mod tests {
    use super::{drain, BATCH_SAMPLES};

    #[test]
    fn draining_takes_what_is_there_and_stops() {
        let (mut producer, consumer) = rtrb::RingBuffer::<f32>::new(64);
        for n in 0..10 {
            producer.push(n as f32).unwrap();
        }

        let mut consumer = consumer;
        let mut batch = Vec::new();
        drain(&mut consumer, &mut batch);

        assert_eq!(batch.len(), 10);
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

        assert_eq!(batch.len(), BATCH_SAMPLES);
    }
}
