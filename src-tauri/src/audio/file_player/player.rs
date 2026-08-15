use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
// Removed unused import
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Represents a single track in the queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedTrack {
    pub id: String,
    pub file_path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<Duration>,
    pub file_size: u64,
    pub added_at: chrono::DateTime<chrono::Utc>,
}

/// Current playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// Playback mode settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackMode {
    pub repeat_mode: RepeatMode,
    pub shuffle: bool,
    pub crossfade_duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatMode {
    None,
    Track,
    Queue,
}

/// Current playback status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackStatus {
    pub state: PlaybackState,
    pub current_track: Option<QueuedTrack>,
    pub position: Duration,
    pub volume: f32, // 0.0 to 1.0
    pub queue_length: usize,
    pub mode: PlaybackMode,
}

/// What is known about the track currently open, beyond its decoder
///
/// Seeking needs the stream to seek within and the timebase to say where it
/// landed; rebuilding the resampler afterwards needs the rate it was built for.
struct LoadedTrack {
    stream_id: u32,
    time_base: Option<symphonia::core::units::TimeBase>,
    input_sample_rate: u32,
}

/// How far into a track "previous" stops meaning the one before
///
/// The convention every transport uses: once you are properly into a track,
/// previous takes you to its start, and only pressing again steps back.
const RESTART_WINDOW: Duration = Duration::from_secs(3);

/// Steps back the history keeps
///
/// Bounded because a player left running all night would otherwise grow one
/// entry per track forever, and nobody steps back through a whole evening.
const HISTORY_LIMIT: usize = 128;

/// Something worth writing down that happened while playing
#[derive(Debug, Clone)]
pub enum PlayerEvent {
    /// The player left this track, having played it or been skipped past it
    TrackFinished { track_id: String },
}

/// Audio file player that decodes files and provides audio samples
pub struct AudioFilePlayer {
    // Queue management
    queue: Arc<Mutex<VecDeque<QueuedTrack>>>,
    current_track_index: Arc<Mutex<Option<usize>>>,

    // Playback state
    state: Arc<Mutex<PlaybackState>>,
    volume: Arc<Mutex<f32>>,
    position: Arc<Mutex<Duration>>,
    mode: Arc<Mutex<PlaybackMode>>,

    // Audio processing
    sample_rate: u32,
    channels: u16,

    // Current decoder and format reader
    current_decoder: Arc<Mutex<Option<Box<dyn Decoder>>>>,
    current_reader: Arc<Mutex<Option<Box<dyn FormatReader>>>>,

    // Resampler for format conversion
    resampler: Arc<Mutex<Option<SincFixedIn<f32>>>>,

    /// Decoded audio the caller has not taken yet, at this player's rate and
    /// layout. A packet decodes to however many frames it holds, which is not
    /// the number asked for, so the remainder waits here for the next call.
    pending: Arc<Mutex<VecDeque<f32>>>,

    /// Frames read from the file but not yet resampled, one queue per channel
    ///
    /// The resampler takes a fixed chunk, so short packets accumulate here until
    /// there is a whole one to give it.
    resample_input: Arc<Mutex<Vec<VecDeque<f32>>>>,

    /// What the loaded track needs for seeking and for reading its own clock
    loaded: Arc<Mutex<Option<LoadedTrack>>>,

    /// Queue positions in the order they were entered, newest last
    ///
    /// Stepping back has to follow what was played rather than what is next to
    /// it in the queue, or shuffle would send "previous" somewhere the listener
    /// has never been.
    played_history: Arc<Mutex<Vec<usize>>>,

    /// Where finished tracks are reported, so history can be written down
    ///
    /// A channel rather than a call, because tracks finish on the decoding
    /// thread, which is a plain thread with no runtime to write to a database
    /// from and no business waiting on one mid-block.
    events: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<PlayerEvent>>>>,
}

impl AudioFilePlayer {
    /// Create a new audio file player
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            current_track_index: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(PlaybackState::Stopped)),
            volume: Arc::new(Mutex::new(1.0)),
            position: Arc::new(Mutex::new(Duration::ZERO)),
            mode: Arc::new(Mutex::new(PlaybackMode {
                repeat_mode: RepeatMode::None,
                shuffle: false,
                crossfade_duration: Duration::from_millis(500),
            })),
            sample_rate,
            channels,
            current_decoder: Arc::new(Mutex::new(None)),
            current_reader: Arc::new(Mutex::new(None)),
            resampler: Arc::new(Mutex::new(None)),
            pending: Arc::new(Mutex::new(VecDeque::new())),
            resample_input: Arc::new(Mutex::new(Vec::new())),
            events: Arc::new(Mutex::new(None)),
            loaded: Arc::new(Mutex::new(None)),
            played_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Send finished tracks here. Without one, playing simply reports nothing.
    pub fn set_event_sender(&self, sender: tokio::sync::mpsc::UnboundedSender<PlayerEvent>) {
        *self.events.lock().unwrap() = Some(sender);
    }

    /// Put an already-built track on the end of the queue
    ///
    /// Used where the track exists before the player sees it — restoring a saved
    /// queue, or queueing a file that has just been written to the database, so
    /// the id in the queue is the id of the row it came from.
    pub fn enqueue(&self, track: QueuedTrack) {
        self.queue.lock().unwrap().push_back(track);
    }

    /// How the queue is played: repeat and shuffle
    pub fn set_mode(&self, repeat_mode: RepeatMode, shuffle: bool) {
        let mut mode = self.mode.lock().unwrap();
        mode.repeat_mode = repeat_mode;
        mode.shuffle = shuffle;
    }

    /// The rate and layout this player emits, whatever the files in its queue are
    pub fn output_format(&self) -> (u32, u16) {
        (self.sample_rate, self.channels)
    }

    /// Add a track to the queue
    pub async fn add_track<P: AsRef<Path>>(&self, file_path: P) -> Result<String> {
        let path = file_path.as_ref().to_path_buf();

        // Validate file exists and get metadata
        let metadata = tokio::fs::metadata(&path)
            .await
            .context("Failed to read file metadata")?;

        if !metadata.is_file() {
            return Err(anyhow::anyhow!("Path is not a file"));
        }

        // Extract audio metadata using symphonia
        let (title, artist, album, duration) = self.extract_metadata(&path).await?;

        let track = QueuedTrack {
            id: uuid::Uuid::new_v4().to_string(),
            file_path: path,
            title,
            artist,
            album,
            duration,
            file_size: metadata.len(),
            added_at: chrono::Utc::now(),
        };

        let track_id = track.id.clone();

        // Add to queue
        {
            let mut queue = self.queue.lock().unwrap();
            queue.push_back(track);
        }

        println!("📀 Added track to queue: {:?}", track_id);
        Ok(track_id)
    }

    /// Remove a track from the queue
    pub fn remove_track(&self, track_id: &str) -> Result<()> {
        let mut queue = self.queue.lock().unwrap();
        let original_len = queue.len();

        queue.retain(|track| track.id != track_id);

        if queue.len() == original_len {
            return Err(anyhow::anyhow!("Track not found in queue"));
        }

        println!("🗑️ Removed track from queue: {}", track_id);
        Ok(())
    }

    /// Start playback
    pub fn play(&self) -> Result<()> {
        let current_state = {
            let state = self.state.lock().unwrap();
            *state
        };

        match current_state {
            PlaybackState::Stopped => {
                // A stopped player is still holding its track at the start, so
                // this resumes that one. Only a player with nothing open goes to
                // the front of the queue.
                if self.current_reader.lock().unwrap().is_none() {
                    self.load_next_track()?;
                }

                let mut state = self.state.lock().unwrap();
                *state = PlaybackState::Playing;
            }
            PlaybackState::Paused => {
                // Resume playback
                let mut state = self.state.lock().unwrap();
                *state = PlaybackState::Playing;
            }
            PlaybackState::Playing => {
                // Already playing
                return Ok(());
            }
        }

        println!("▶️ Started playback");
        Ok(())
    }

    /// Pause playback
    pub fn pause(&self) {
        let mut state = self.state.lock().unwrap();
        if *state == PlaybackState::Playing {
            *state = PlaybackState::Paused;
            println!("⏸️ Paused playback");
        }
    }

    /// Stop playback, holding the current track at its start
    ///
    /// Stopped is not unloaded. The track stays open at 0:00 so play starts it
    /// again — dropping the decoder here meant play fell through to loading the
    /// front of the queue, so stopping halfway through the third ad and pressing
    /// play went back to the first one.
    pub fn stop(&self) {
        {
            let mut state = self.state.lock().unwrap();
            *state = PlaybackState::Stopped;
        }

        // Rewinds the reader, resets the decoder and drops everything buffered
        // past this point. Nothing loaded is not a failure worth reporting from
        // a stop — there is simply nothing to rewind.
        let _ = self.seek(Duration::ZERO);
        *self.position.lock().unwrap() = Duration::ZERO;

        println!("⏹️ Stopped playback");
    }

    /// The queue played out
    ///
    /// Distinct from stopping, which holds the track it was on. Nothing is held
    /// here, so playing again runs the queue from the top — stopping halfway
    /// through an ad break should resume that ad, but a break that finished
    /// should start over rather than repeat its last spot.
    pub fn finish_queue(&self) {
        self.stop();

        *self.current_decoder.lock().unwrap() = None;
        *self.current_reader.lock().unwrap() = None;
        *self.loaded.lock().unwrap() = None;
        *self.current_track_index.lock().unwrap() = None;
        self.played_history.lock().unwrap().clear();

        println!("⏹️ Queue finished");
    }

    /// Skip to next track
    pub fn skip_next(&self) -> Result<()> {
        // Moves on rather than restarting, and reports the queue having played
        // out rather than treating it as a failure.
        if !self.advance_track()? {
            self.stop();
            println!("⏭️ Queue finished");
            return Ok(());
        }

        println!("⏭️ Skipped to next track");
        Ok(())
    }

    /// Skip to previous track
    /// Go back: to the start of this track, or to the one before it
    ///
    /// Which one depends on how far in you are. Past the first few seconds,
    /// previous means the start of what is playing — pressing it again, now at
    /// zero, is what steps back a track. It follows what was actually played
    /// rather than queue order, so it works the same under shuffle.
    pub fn skip_previous(&self) -> Result<()> {
        let position = *self.position.lock().unwrap();

        if position >= RESTART_WINDOW {
            println!("⏮️ Restarted current track");
            return self.restart_track();
        }

        let previous = self.played_history.lock().unwrap().pop();

        match previous {
            Some(index) => {
                self.load_index(index, false)?;
                println!("⏮️ Skipped to previous track");
            }
            // Nothing has been played before this, so the start of it is as far
            // back as there is to go.
            None => {
                self.restart_track()?;
                println!("⏮️ Nothing before this, restarted it");
            }
        }

        Ok(())
    }

    /// Set playback volume (0.0 to 1.0)
    pub fn set_volume(&self, volume: f32) {
        let mut vol = self.volume.lock().unwrap();
        *vol = volume.clamp(0.0, 1.0);
    }

    /// Get current playback status
    pub fn get_status(&self) -> PlaybackStatus {
        let queue = self.queue.lock().unwrap();
        let state = *self.state.lock().unwrap();
        let position = *self.position.lock().unwrap();
        let volume = *self.volume.lock().unwrap();
        let mode = self.mode.lock().unwrap().clone();

        let current_track = if let Some(index) = *self.current_track_index.lock().unwrap() {
            queue.get(index).cloned()
        } else {
            None
        };

        PlaybackStatus {
            state,
            current_track,
            position,
            volume,
            queue_length: queue.len(),
            mode,
        }
    }

    /// Get queue contents
    pub fn get_queue(&self) -> Vec<QueuedTrack> {
        self.queue.lock().unwrap().iter().cloned().collect()
    }

    /// Take up to `frames` frames of audio, interleaved at this player's format
    ///
    /// Returns fewer frames than asked for when the track ends mid-block, and
    /// `None` once the queue is exhausted and nothing is left buffered — which
    /// is what tells the caller playback has finished rather than stalled.
    ///
    /// Paused and stopped return `Some(empty)`: the source is still attached and
    /// its channel strip still exists, it simply has nothing to say. Reporting
    /// the end would tear the input down every time someone hit pause.
    pub fn next_block(&self, frames: usize) -> Result<Option<Vec<f32>>> {
        let wanted = frames * self.channels as usize;

        if *self.state.lock().unwrap() != PlaybackState::Playing {
            return Ok(Some(Vec::new()));
        }

        // A track that decodes to nothing must not send this round the queue
        // forever, so one call moves on at most as many times as there are
        // tracks to move to.
        let mut advances_left = self.queue.lock().unwrap().len() + 1;

        while self.pending.lock().unwrap().len() < wanted {
            if self.decode_one_packet()? {
                continue;
            }

            // The file ran out. The next one picks up within the same block, so
            // a playlist plays as one continuous stream rather than dropping a
            // chunk of silence at every join.
            if advances_left == 0 || !self.advance_track()? {
                break;
            }
            advances_left -= 1;
        }

        let mut pending = self.pending.lock().unwrap();
        if pending.is_empty() {
            return Ok(None);
        }

        let take = wanted.min(pending.len());
        let volume = *self.volume.lock().unwrap();
        let block: Vec<f32> = pending
            .drain(..take)
            .map(|sample| sample * volume)
            .collect();
        drop(pending);

        self.advance_position(take / self.channels as usize);

        Ok(Some(block))
    }

    /// Move the playhead on by what was just handed out
    ///
    /// Counted from what leaves rather than from a clock: the two only agree
    /// while audio is actually flowing, and it is the audio the reading is
    /// about. A paused player hands out nothing and its playhead stays put.
    fn advance_position(&self, frames: usize) {
        let elapsed = Duration::from_secs_f64(frames as f64 / self.sample_rate as f64);
        *self.position.lock().unwrap() += elapsed;
    }

    /// Decode one packet into `pending`. False once the file has no more.
    fn decode_one_packet(&self) -> Result<bool> {
        let mut reader_guard = self.current_reader.lock().unwrap();
        let mut decoder_guard = self.current_decoder.lock().unwrap();

        let (Some(reader), Some(decoder)) = (reader_guard.as_mut(), decoder_guard.as_mut()) else {
            return Ok(false);
        };

        let packet = match reader.next_packet() {
            Ok(packet) => packet,
            // Every error out of `next_packet` ends this file: end of stream,
            // a reset the decoder cannot follow, or a read failure.
            Err(_) => return Ok(false),
        };

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // A single bad packet is worth skipping rather than ending the
            // track, which is what symphonia's own examples do.
            Err(symphonia::core::errors::Error::DecodeError(_)) => return Ok(true),
            Err(e) => return Err(anyhow::anyhow!("decode failed: {}", e)),
        };

        let spec = *decoded.spec();
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);

        let mapped = map_channels(
            buffer.samples(),
            spec.channels.count(),
            self.channels as usize,
        );

        if self.resampler.lock().unwrap().is_some() {
            self.push_for_resampling(&mapped)?;
        } else {
            self.pending.lock().unwrap().extend(mapped);
        }

        Ok(true)
    }

    /// Feed already channel-mapped audio through the resampler in whole chunks
    fn push_for_resampling(&self, interleaved: &[f32]) -> Result<()> {
        let channels = self.channels as usize;

        let mut input = self.resample_input.lock().unwrap();
        if input.len() != channels {
            *input = vec![VecDeque::new(); channels];
        }

        for (index, sample) in interleaved.iter().enumerate() {
            input[index % channels].push_back(*sample);
        }

        let mut resampler = self.resampler.lock().unwrap();
        let Some(resampler) = resampler.as_mut() else {
            return Ok(());
        };

        // The resampler takes a fixed chunk and refuses a short one, so it only
        // runs once a whole one has arrived.
        let chunk = resampler.input_frames_next();
        while input[0].len() >= chunk {
            let block: Vec<Vec<f32>> = input
                .iter_mut()
                .map(|channel| channel.drain(..chunk).collect())
                .collect();

            let resampled = resampler
                .process(&block, None)
                .context("Failed to resample decoded audio")?;

            let mut pending = self.pending.lock().unwrap();
            for frame in 0..resampled[0].len() {
                for channel in resampled.iter() {
                    pending.push_back(channel[frame]);
                }
            }
        }

        Ok(())
    }

    /// Load the track at the front of the queue, starting playback from the top
    fn load_next_track(&self) -> Result<()> {
        if self.queue.lock().unwrap().is_empty() {
            return Err(anyhow::anyhow!("Queue is empty"));
        }

        self.played_history.lock().unwrap().clear();
        self.load_index(0, false)
    }

    /// Move to whatever should play after the current track
    ///
    /// False once the queue has played out, which is what tells the decoder to
    /// stop rather than to keep asking for packets from a finished file.
    fn advance_track(&self) -> Result<bool> {
        // Reported before moving on, so a queue that plays out still records its
        // last track rather than dropping the one nothing followed.
        self.report_finished();

        let Some(next) = self.next_index() else {
            return Ok(false);
        };

        self.load_index(next, true)?;
        Ok(true)
    }

    /// Say that the current track is done, if anyone is listening
    ///
    /// Finishing here means leaving the track, which includes being skipped past
    /// it. What was played rather than merely reached is a finer question than
    /// the queue can answer on its own.
    fn report_finished(&self) {
        let Some(index) = *self.current_track_index.lock().unwrap() else {
            return;
        };

        let Some(track_id) = self
            .queue
            .lock()
            .unwrap()
            .get(index)
            .map(|track| track.id.clone())
        else {
            return;
        };

        if let Some(sender) = self.events.lock().unwrap().as_ref() {
            // A closed receiver means nothing is recording history any more,
            // which is not a reason to interrupt playback.
            let _ = sender.send(PlayerEvent::TrackFinished { track_id });
        }
    }

    /// Which track follows the current one, or None when the queue is done
    fn next_index(&self) -> Option<usize> {
        let queue_length = self.queue.lock().unwrap().len();
        if queue_length == 0 {
            return None;
        }

        let current = (*self.current_track_index.lock().unwrap()).unwrap_or(0);
        let mode = self.mode.lock().unwrap().clone();

        // Repeating one track outranks shuffle: asking for the same track over
        // and over and getting a random one instead would be nobody's reading.
        if mode.repeat_mode == RepeatMode::Track {
            return Some(current);
        }

        if mode.shuffle {
            return Some(self.pick_shuffled(queue_length, current));
        }

        let next = current + 1;
        if next < queue_length {
            Some(next)
        } else if mode.repeat_mode == RepeatMode::Queue {
            Some(0)
        } else {
            None
        }
    }

    /// A different track at random
    ///
    /// Seeded from the clock rather than from a generator crate, which this is
    /// not worth taking on: nothing here needs the sequence to be unguessable,
    /// only for two ads in a row not to be the same one.
    fn pick_shuffled(&self, queue_length: usize, current: usize) -> usize {
        if queue_length == 1 {
            return 0;
        }

        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.subsec_nanos() as u64)
            .unwrap_or(1)
            | 1;

        // xorshift64, enough to spread a playlist
        let mut state = seed;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;

        // Drawn from the tracks that are not the current one, so a shuffle
        // always moves
        let offset = (state % (queue_length as u64 - 1)) as usize;
        (current + 1 + offset) % queue_length
    }

    /// Load the track at `index` and make it the current one
    ///
    /// `remember` records the track being left, which is what "previous" walks
    /// back through. Stepping back and restarting both pass false: neither is
    /// somewhere new to come back from.
    fn load_index(&self, index: usize, remember: bool) -> Result<()> {
        let track = {
            let queue = self.queue.lock().unwrap();
            queue
                .get(index)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("No track at index {}", index))?
        };

        if remember {
            if let Some(leaving) = *self.current_track_index.lock().unwrap() {
                let mut history = self.played_history.lock().unwrap();
                history.push(leaving);
                if history.len() > HISTORY_LIMIT {
                    history.remove(0);
                }
            }
        }

        self.load_track(&track)?;
        *self.current_track_index.lock().unwrap() = Some(index);

        Ok(())
    }

    /// Play the current track again from its start
    pub fn restart_track(&self) -> Result<()> {
        let Some(index) = *self.current_track_index.lock().unwrap() else {
            return Err(anyhow::anyhow!("Nothing is loaded"));
        };

        self.load_index(index, false)
    }

    /// Move the playhead within the current track
    ///
    /// Lands on the nearest point the format can start decoding from rather than
    /// exactly where it was asked, so the reported position is read back from
    /// where it actually landed instead of being assumed.
    pub fn seek(&self, target: Duration) -> Result<()> {
        let loaded = self
            .loaded
            .lock()
            .unwrap()
            .as_ref()
            .map(|track| (track.stream_id, track.time_base, track.input_sample_rate))
            .ok_or_else(|| anyhow::anyhow!("Nothing is loaded to seek in"))?;
        let (stream_id, time_base, input_sample_rate) = loaded;

        let landed = {
            let mut reader_guard = self.current_reader.lock().unwrap();
            let reader = reader_guard
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("Nothing is loaded to seek in"))?;

            reader
                .seek(
                    symphonia::core::formats::SeekMode::Accurate,
                    symphonia::core::formats::SeekTo::Time {
                        time: symphonia::core::units::Time::from(target.as_secs_f64()),
                        track_id: Some(stream_id),
                    },
                )
                .context("Could not seek in this track")?
        };

        // The decoder holds state from before the jump, and so does everything
        // downstream of it: leaving any of it in place would be heard as a
        // moment of the old position after the new one.
        if let Some(decoder) = self.current_decoder.lock().unwrap().as_mut() {
            decoder.reset();
        }
        self.pending.lock().unwrap().clear();
        self.resample_input.lock().unwrap().clear();
        self.configure_resampler(input_sample_rate)?;

        *self.position.lock().unwrap() = match time_base {
            Some(base) => {
                let time = base.calc_time(landed.actual_ts);
                Duration::from_secs_f64(time.seconds as f64 + time.frac)
            }
            // No timebase to convert with, so the asked-for position is the best
            // reading available.
            None => target,
        };

        Ok(())
    }

    /// Load a specific track for playback
    fn load_track(&self, track: &QueuedTrack) -> Result<()> {
        println!("🎵 Loading track: {:?}", track.file_path);

        // Open the file
        let file = std::fs::File::open(&track.file_path).context("Failed to open audio file")?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Create a probe hint using the file extension
        let mut hint = Hint::new();
        if let Some(extension) = track.file_path.extension() {
            if let Some(extension_str) = extension.to_str() {
                hint.with_extension(extension_str);
            }
        }

        // Use the default options for metadata and format readers
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        // Probe the media source
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .context("Unsupported format")?;

        // Get the instantiated format reader
        let mut format = probed.format;

        // Find the first audio track with a known (decodeable) codec
        let track_info = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .context("No supported audio tracks")?;

        let track_id = track_info.id;
        let track_info_cloned = track_info.clone();

        // Use the default options for the decoder
        let dec_opts: DecoderOptions = Default::default();

        // Create a decoder for the track
        let decoder = symphonia::default::get_codecs()
            .make(&track_info.codec_params, &dec_opts)
            .context("Unsupported codec")?;

        // Store the decoder and reader
        {
            let mut current_decoder = self.current_decoder.lock().unwrap();
            *current_decoder = Some(decoder);
        }

        {
            let mut current_reader = self.current_reader.lock().unwrap();
            *current_reader = Some(format);
        }

        // A new track starts from silence rather than from whatever the last one
        // left mid-chunk, which would otherwise be heard as a click on the join.
        self.pending.lock().unwrap().clear();
        self.resample_input.lock().unwrap().clear();
        *self.position.lock().unwrap() = Duration::ZERO;

        // Rate conversion only. Channels are mapped before the resampler runs,
        // so it is always configured for the player's own layout and does not
        // have to be rebuilt when the next track has a different one.
        let input_sample_rate = track_info_cloned
            .codec_params
            .sample_rate
            .unwrap_or(crate::types::DEFAULT_SAMPLE_RATE);

        self.configure_resampler(input_sample_rate)?;

        *self.loaded.lock().unwrap() = Some(LoadedTrack {
            stream_id: track_id,
            time_base: track_info_cloned.codec_params.time_base,
            input_sample_rate,
        });

        println!("✅ Track loaded successfully");
        Ok(())
    }

    /// Build the resampler this track needs, or none when the rates already match
    ///
    /// Called again after a seek: the resampler carries filter state across
    /// chunks, and state from before a jump would be heard after it.
    fn configure_resampler(&self, input_sample_rate: u32) -> Result<()> {
        let mut current_resampler = self.resampler.lock().unwrap();

        *current_resampler = if input_sample_rate == self.sample_rate {
            None
        } else {
            let params = SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                oversampling_factor: 256,
                interpolation: SincInterpolationType::Linear,
                window: WindowFunction::BlackmanHarris2,
            };

            Some(
                SincFixedIn::<f32>::new(
                    self.sample_rate as f64 / input_sample_rate as f64,
                    2.0, // Max relative change in sample rate
                    params,
                    1024, // Chunk size
                    self.channels as usize,
                )
                .context("Failed to create resampler")?,
            )
        };

        Ok(())
    }

    /// Extract metadata from an audio file
    async fn extract_metadata(
        &self,
        path: &Path,
    ) -> Result<(
        Option<String>,
        Option<String>,
        Option<String>,
        Option<Duration>,
    )> {
        // For now, return None for all metadata
        // TODO: Implement proper metadata extraction using symphonia
        Ok((None, None, None, None))
    }
}

/// Fit interleaved audio to a channel count, so every track leaves the player
/// in the layout it declared however the file was recorded
///
/// Mono into stereo is duplicated rather than panned to one side, and a wider
/// file is folded down by averaging, which keeps a centre image rather than
/// discarding whatever is not in the first channels.
fn map_channels(samples: &[f32], from: usize, to: usize) -> Vec<f32> {
    if from == to || from == 0 {
        return samples.to_vec();
    }

    let frames = samples.len() / from;
    let mut mapped = Vec::with_capacity(frames * to);

    for frame in 0..frames {
        let source = &samples[frame * from..frame * from + from];

        if from == 1 {
            mapped.extend(std::iter::repeat(source[0]).take(to));
        } else if to == 1 {
            mapped.push(source.iter().sum::<f32>() / from as f32);
        } else {
            // Take what lines up and average the rest into the last channel, so
            // a 5.1 file still reaches both sides of a stereo player.
            for channel in 0..to {
                if channel + 1 == to && from > to {
                    let rest = &source[channel..];
                    mapped.push(rest.iter().sum::<f32>() / rest.len() as f32);
                } else {
                    mapped.push(*source.get(channel).unwrap_or(&0.0));
                }
            }
        }
    }

    mapped
}

/// Represents a virtual audio device that streams from the file player
pub struct FilePlayerDevice {
    player: Arc<AudioFilePlayer>,
    device_name: String,
}

impl FilePlayerDevice {
    pub fn new(device_name: String, sample_rate: u32, channels: u16) -> Self {
        // Deliberately no id of its own. The manager's key is the identifier a
        // player is known by everywhere — what `create_player` returns, what the
        // device list advertises, and what a channel is patched to — and a
        // second one here only ever disagreed with it.
        Self {
            player: Arc::new(AudioFilePlayer::new(sample_rate, channels)),
            device_name,
        }
    }

    pub fn get_device_name(&self) -> &str {
        &self.device_name
    }

    pub fn get_player(&self) -> Arc<AudioFilePlayer> {
        self.player.clone()
    }
}
