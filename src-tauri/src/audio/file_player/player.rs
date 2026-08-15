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
        }
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
    pub async fn play(&self) -> Result<()> {
        let current_state = {
            let state = self.state.lock().unwrap();
            *state
        };

        match current_state {
            PlaybackState::Stopped => {
                // Start playing first track in queue
                self.load_next_track().await?;
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

    /// Stop playback
    pub fn stop(&self) {
        {
            let mut state = self.state.lock().unwrap();
            *state = PlaybackState::Stopped;
        }

        // Reset position
        {
            let mut position = self.position.lock().unwrap();
            *position = Duration::ZERO;
        }

        // Clear current decoder
        {
            let mut decoder = self.current_decoder.lock().unwrap();
            *decoder = None;
        }

        {
            let mut reader = self.current_reader.lock().unwrap();
            *reader = None;
        }

        println!("⏹️ Stopped playback");
    }

    /// Skip to next track
    pub async fn skip_next(&self) -> Result<()> {
        self.load_next_track().await?;
        println!("⏭️ Skipped to next track");
        Ok(())
    }

    /// Skip to previous track
    pub async fn skip_previous(&self) -> Result<()> {
        // For now, just restart current track
        // TODO: Implement proper previous track logic
        {
            let mut position = self.position.lock().unwrap();
            *position = Duration::ZERO;
        }
        println!("⏮️ Skipped to previous track");
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

        while self.pending.lock().unwrap().len() < wanted {
            if !self.decode_one_packet()? {
                break;
            }
        }

        let mut pending = self.pending.lock().unwrap();
        if pending.is_empty() {
            return Ok(None);
        }

        let take = wanted.min(pending.len());
        let volume = *self.volume.lock().unwrap();

        Ok(Some(
            pending.drain(..take).map(|sample| sample * volume).collect(),
        ))
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

        let mapped = map_channels(buffer.samples(), spec.channels.count(), self.channels as usize);

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

    /// Load and start playing the next track
    async fn load_next_track(&self) -> Result<()> {
        let track = {
            let queue = self.queue.lock().unwrap();

            if queue.is_empty() {
                return Err(anyhow::anyhow!("Queue is empty"));
            }

            // For now, just play first track
            // TODO: Implement proper next track logic with shuffle/repeat
            queue.front().unwrap().clone()
        };

        self.load_track(&track).await?;

        let mut current_index = self.current_track_index.lock().unwrap();
        *current_index = Some(0);

        Ok(())
    }

    /// Load a specific track for playback
    async fn load_track(&self, track: &QueuedTrack) -> Result<()> {
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

        // Rate conversion only. Channels are mapped before the resampler runs,
        // so it is always configured for the player's own layout and does not
        // have to be rebuilt when the next track has a different one.
        let input_sample_rate = track_info_cloned
            .codec_params
            .sample_rate
            .unwrap_or(crate::types::DEFAULT_SAMPLE_RATE);

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

        println!("✅ Track loaded successfully");
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
    device_id: String,
    device_name: String,
}

impl FilePlayerDevice {
    pub fn new(device_name: String, sample_rate: u32, channels: u16) -> Self {
        let device_id = format!("file_player_{}", uuid::Uuid::new_v4());
        let player = Arc::new(AudioFilePlayer::new(sample_rate, channels));

        Self {
            player,
            device_id,
            device_name,
        }
    }

    pub fn get_device_id(&self) -> &str {
        &self.device_id
    }

    pub fn get_device_name(&self) -> &str {
        &self.device_name
    }

    pub fn get_player(&self) -> Arc<AudioFilePlayer> {
        self.player.clone()
    }
}
