// Layer 4: Output Processing Workers
//
// Each output device gets its own dedicated worker thread that:
// 1. Receives mixed audio from Layer 3 mixing
// 2. Resamples from max rate to device-specific rate
// 3. Buffers samples to proper chunk sizes for hardware
// 4. Sends audio to actual output devices

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, trace, warn};

use super::queue_types::MixedAudioSamples;
use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use crate::audio::mixer::resampling::RubatoSRC;
use crate::audio::utils::calculate_optimal_chunk_size;
use colored::*;

// RTRB queue imports for hardware output
use rtrb::Producer;

/// Output processing worker for a specific device
pub struct OutputWorker {
    device_id: String,
    pub device_sample_rate: u32, // Target device sample rate (e.g., 44.1kHz)
    channels: u16,               // Output device channel count (mono/stereo/etc)

    // Audio processing components
    resampler: Option<RubatoSRC>,
    sample_buffer: Vec<f32>,  // Hardware chunk accumulator
    target_chunk_size: usize, // Device-required buffer size (e.g., 512 samples stereo)

    // **ACCUMULATION**: Buffer for collecting variable FftFixedIn outputs until hardware chunk size
    accumulation_buffer: Vec<f32>, // Accumulates samples until target_chunk_size reached

    // Communication channels
    mixed_audio_rx: mpsc::UnboundedReceiver<MixedAudioSamples>,

    // Hardware buffer size updates (macOS CoreAudio only)
    #[cfg(target_os = "macos")]
    hardware_update_tx: Option<mpsc::Sender<crate::audio::mixer::stream_management::AudioCommand>>,

    // Hardware output integration via RTRB queue
    rtrb_producer: Option<Arc<Mutex<Producer<f32>>>>, // Writes to hardware via RTRB queue

    // Queue state tracking for dynamic sample rate adjustment
    queue_tracker: AtomicQueueTracker,

    // Worker thread handle
    worker_handle: Option<tokio::task::JoinHandle<()>>,

    // Performance metrics
    chunks_processed: u64,
    samples_output: u64,
}

impl OutputWorker {
    /// Create a new output processing worker with RTRB producer and queue tracker
    pub fn new_with_rtrb_producer_and_tracker(
        device_id: String,
        device_sample_rate: u32,
        target_chunk_size: usize,
        channels: u16, // Output device channel count (mono/stereo/etc)
        mixed_audio_rx: mpsc::UnboundedReceiver<MixedAudioSamples>,
        rtrb_producer: Option<Arc<Mutex<Producer<f32>>>>,
        queue_tracker: AtomicQueueTracker,
    ) -> Self {
        let has_hardware_output = rtrb_producer.is_some();
        info!(
            "🔊 OUTPUT_WORKER: Creating worker for device '{}' ({} Hz, {} sample chunks, hardware: {})",
            device_id, device_sample_rate, target_chunk_size, has_hardware_output
        );

        Self {
            device_id,
            device_sample_rate,
            channels,
            resampler: None,
            sample_buffer: Vec::new(),
            target_chunk_size,
            accumulation_buffer: Vec::with_capacity(target_chunk_size * 2), // Pre-allocate for efficiency
            mixed_audio_rx,
            #[cfg(target_os = "macos")]
            hardware_update_tx: None, // No hardware updates for this constructor
            rtrb_producer: rtrb_producer,
            queue_tracker,
            worker_handle: None,
            chunks_processed: 0,
            samples_output: 0,
        }
    }

    /// Create a new output processing worker with hardware update channel (macOS only)
    #[cfg(target_os = "macos")]
    pub fn new_with_hardware_updates(
        device_id: String,
        device_sample_rate: u32,
        target_chunk_size: usize,
        channels: u16, // Output device channel count (mono/stereo/etc)
        mixed_audio_rx: mpsc::UnboundedReceiver<MixedAudioSamples>,
        rtrb_producer: Option<Arc<Mutex<Producer<f32>>>>,
        hardware_update_tx: mpsc::Sender<crate::audio::mixer::stream_management::AudioCommand>,
        queue_tracker: AtomicQueueTracker,
    ) -> Self {
        let has_hardware_output = rtrb_producer.is_some();
        info!(
            "🔊 OUTPUT_WORKER: Creating worker with hardware updates for device '{}' ({} Hz, {} sample chunks, hardware: {})",
            device_id, device_sample_rate, target_chunk_size, has_hardware_output
        );

        Self {
            device_id,
            device_sample_rate,
            channels,
            resampler: None,
            sample_buffer: Vec::new(),
            target_chunk_size,
            accumulation_buffer: Vec::with_capacity(target_chunk_size * 2), // Pre-allocate for efficiency
            mixed_audio_rx,
            hardware_update_tx: Some(hardware_update_tx),
            rtrb_producer: rtrb_producer,
            queue_tracker,
            worker_handle: None,
            chunks_processed: 0,
            samples_output: 0,
        }
    }

    /// Update the target mix rate, using dynamic rate adjustment if supported
    pub fn update_target_mix_rate(&mut self, target_mix_rate: u32) -> Result<()> {
        if let Some(ref mut resampler) = self.resampler {
            // Check if current resampler supports dynamic rate adjustment
            if resampler.supports_dynamic_sample_rate() {
                // Use dynamic adjustment - keep same output rate, update input rate
                let output_rate = self.device_sample_rate as f32;
                let new_input_rate = target_mix_rate as f32;

                match resampler.set_sample_rates(new_input_rate, output_rate, true) {
                    Ok(()) => {
                        info!(
                            "🎯 {}: Dynamic rate update for {} - {}Hz→{}Hz (ratio: {:.6})",
                            "DYNAMIC_RATE_UPDATE".cyan(),
                            self.device_id,
                            new_input_rate,
                            output_rate,
                            output_rate / new_input_rate
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        warn!(
                            "⚠️ {}: Dynamic rate update failed for {}: {}, falling back to recreation",
                            "DYNAMIC_RATE_FAILED".yellow(),
                            self.device_id,
                            e
                        );
                        // Fall through to recreation
                    }
                }
            } else {
                info!(
                    "🔄 {}: Resampler for {} doesn't support dynamic rates, recreating",
                    "RESAMPLER_RECREATION".blue(),
                    self.device_id
                );
            }
        }

        // Fallback: force resampler recreation on next processing cycle
        self.resampler = None;
        info!(
            "🔧 {}: Marked resampler for recreation - new mix rate: {}Hz",
            "RESAMPLER_RESET".blue(),
            target_mix_rate
        );
        Ok(())
    }

    /// Get queue tracker for sharing with CoreAudio callback
    pub fn get_queue_tracker_for_consumer(&self) -> AtomicQueueTracker {
        self.queue_tracker.clone()
    }

    /// Static helper function to get or initialize resampler in async context
    fn get_or_initialize_resampler_static<'a>(
        resampler: &'a mut Option<RubatoSRC>,
        input_sample_rate: u32,
        output_sample_rate: u32,
        chunk_size: usize, // Output device chunk size
        device_id: &str,
    ) -> Option<&'a mut RubatoSRC> {
        let sample_rate_difference = (input_sample_rate as f32 - output_sample_rate as f32).abs();

        // No resampling needed if rates are close (within 1 Hz)
        if sample_rate_difference <= 1.0 {
            return None;
        }

        // Check if we need to recreate the resampler or can use dynamic adjustment
        let (needs_recreation, can_adjust_dynamically) =
            if let Some(ref existing_resampler) = resampler {
                let rates_changed = existing_resampler.input_rate() != input_sample_rate
                    || existing_resampler.output_rate() != output_sample_rate;

                if rates_changed && existing_resampler.supports_dynamic_sample_rate() {
                    (false, true) // Can adjust dynamically, no recreation needed
                } else if rates_changed {
                    (true, false) // Need recreation, no dynamic support
                } else {
                    (false, false) // No change needed
                }
            } else {
                (true, false) // No resampler exists, need to create
            };

        // **DYNAMIC RATIO ADJUSTMENT**: Try to adjust existing resampler first
        if can_adjust_dynamically {
            if let Some(ref mut existing_resampler) = resampler {
                match existing_resampler.set_sample_rates(
                    input_sample_rate as f32,
                    output_sample_rate as f32,
                    true, // Use ramping for smooth transitions
                ) {
                    Ok(()) => {}
                    Err(e) => {
                        info!(
                            "⚠️ {}: Dynamic adjustment failed: {} - falling back to recreation",
                            "DYNAMIC_ADJUST_FALLBACK".yellow(),
                            e
                        );
                        // Fall back to recreation
                        *resampler = None;
                    }
                }
            }
        }

        // Create or recreate resampler if needed
        if needs_recreation {
            // **CLOCK SYNC RESAMPLER**: Use SincFixedOut for dynamic ratio adjustment
            match RubatoSRC::new_sinc_fixed_output(
                input_sample_rate as f32,
                output_sample_rate as f32,
                chunk_size / 2,
                2, // Output workers process stereo from mixing → convert to device channels later
            ) {
                // match RubatoSRC::new_fft_fixed_input(
                //     input_sample_rate as f32,
                //     output_sample_rate as f32,
                //     128, // test value to match input sample rate currently hardcoded to 128 frames
                // ) {
                Ok(new_resampler) => {
                    info!(
                        "🔄 {}: {} resampler for {} ({} Hz → {} Hz, ratio: {:.3})",
                        "OUTPUT_RESAMPLER".green(),
                        if resampler.is_some() {
                            "Recreated"
                        } else {
                            "Created"
                        },
                        device_id,
                        input_sample_rate,
                        output_sample_rate,
                        new_resampler.ratio()
                    );
                    *resampler = Some(new_resampler);
                }
                Err(e) => {
                    error!(
                        "❌ OUTPUT_WORKER: Failed to create resampler for {}: {}",
                        device_id, e
                    );
                    return None;
                }
            }
        }

        // Return mutable reference to the resampler
        resampler.as_mut()
    }

    /// Pre-accumulation strategy: Wait until we have enough input to produce required output
    ///
    /// Strategy:
    /// 1. Wait until samples are available, drain all
    /// 2. Add to pre accumulator
    /// 3. While (enough samples to convert to target chunk amount)
    ///    - Convert
    ///    - Write to SPMC
    fn process_with_pre_accumulation(
        resampler: &mut Option<RubatoSRC>,
        input_sample_rate: u32,
        device_sample_rate: u32,
        chunk_size: usize,
        device_id: &str,
        input_samples: &[f32],
        target_output_count: usize,
        pre_accumulation_buffer: &mut Vec<f32>,
        rtrb_producer: &Option<Arc<Mutex<Producer<f32>>>>,
        output_started: &mut bool,
        queue_tracker: Option<&AtomicQueueTracker>,
    ) -> Vec<f32> {
        // Step 1 & 2: Wait until samples are available, drain all and add to pre accumulator
        pre_accumulation_buffer.extend_from_slice(input_samples);

        // Get precise input frames needed from the resampler itself
        let estimated_input_needed = if let Some(active_resampler) =
            Self::get_or_initialize_resampler_static(
                resampler,
                input_sample_rate,
                device_sample_rate,
                chunk_size,
                device_id,
            ) {
            // Use resampler's own calculation for how many input frames it needs
            active_resampler.input_frames_needed(target_output_count / 2) * 2 // Convert frames to samples
        } else {
            // Fallback to manual calculation if no resampling needed
            target_output_count
        };

        // **BUFFER MONITORING**: Track pre-accumulation buffer levels for drift analysis
        static BUFFER_LEVEL_LOG_COUNT: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let buffer_log_count =
            BUFFER_LEVEL_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if buffer_log_count % 1000 == 0 {
            info!(
                "🔄 {}: Buffer levels - accumulated: {}, input needed: {}, ratio: {:.2}",
                "BUFFER_DRIFT_TRACK".cyan(),
                pre_accumulation_buffer.len(),
                estimated_input_needed,
                pre_accumulation_buffer.len() as f32 / estimated_input_needed as f32
            );
        }

        // Start output immediately - no delayed start to prevent queue overflow
        *output_started = true;

        let mut chunks_written = 0;

        // Step 3: Dynamic processing loop - check requirements each iteration
        loop {
            // **DYNAMIC REQUIREMENT**: Get current input requirement from resampler
            let current_input_needed = if let Some(active_resampler) =
                Self::get_or_initialize_resampler_static(
                    resampler,
                    input_sample_rate,
                    device_sample_rate,
                    chunk_size,
                    device_id,
                ) {
                let samples_needed =
                    active_resampler.input_frames_needed(target_output_count / 2) * 2;

                samples_needed
            } else {
                target_output_count
            };

            // Check if we have enough for current requirement
            if pre_accumulation_buffer.len() < current_input_needed {
                break; // Not enough samples yet
            }

            // Convert: Process exactly what the resampler needs right now
            let input_to_process = pre_accumulation_buffer
                .drain(0..current_input_needed)
                .collect::<Vec<_>>();
            let output_chunk = Self::process_resampling_static(
                resampler,
                input_sample_rate,
                device_sample_rate,
                chunk_size,
                device_id,
                &input_to_process,
                target_output_count,
            );

            // Write to RTRB: Immediately write to keep queue fed
            if !output_chunk.is_empty() {
                static OUTPUT_CHUNK_WRITE_COUNT: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let chunk_write_count =
                    OUTPUT_CHUNK_WRITE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                if chunk_write_count < 10 || chunk_write_count % 100 == 0 {
                    info!(
                    "🔄 {}: Chunked processing wrote {} samples ({} remaining) directly to RTRB, {} input frames needed",
                    "CHUNKS_WRITTEN_TO_RTRB".on_blue().yellow(),
                    output_chunk.len(),
                    pre_accumulation_buffer.len(),
                    current_input_needed

                );
                }

                if let Some(ref writer) = rtrb_producer {
                    Self::write_samples_to_rtrb_sync(
                        device_id,
                        &output_chunk,
                        writer,
                        queue_tracker,
                    );
                    chunks_written += 1;
                    Self::adjust_dynamic_sample_rate_static(
                        resampler,
                        queue_tracker,
                        input_sample_rate,
                        device_sample_rate,
                        chunk_size,
                        device_id,
                    );
                }
            }
        }

        // Log chunked processing activity
        if chunks_written > 0 {
            static CHUNK_LOG_COUNT: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let chunk_count = CHUNK_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            if chunk_count < 10 || chunk_count % 100 == 0 || chunks_written > 1 {
                info!(
                    "🔄 {}: Chunked processing wrote {} chunks directly to RTRB ({}Hz→{}Hz)",
                    "PRE_ACCUM_CHUNKS".on_blue().yellow(),
                    chunks_written,
                    input_sample_rate,
                    device_sample_rate
                );
            }
        }

        // Return empty since we already wrote to RTRB queue
        // This avoids double-writing in the main loop
        Vec::new()
    }

    /// Post-accumulation strategy: Process input immediately, accumulate output until target reached
    fn process_with_post_accumulation(
        resampler: &mut Option<RubatoSRC>,
        input_sample_rate: u32,
        device_sample_rate: u32,
        chunk_size: usize,
        device_id: &str,
        input_samples: &[f32],
        target_output_count: usize,
        post_accumulation_buffer: &mut Vec<f32>,
        output_started: &mut bool,
        queue_tracker: Option<&AtomicQueueTracker>,
    ) -> Vec<f32> {
        // Process input immediately, get whatever output is available
        let resampled = Self::process_resampling_static(
            resampler,
            input_sample_rate,
            device_sample_rate,
            chunk_size,
            device_id,
            input_samples,
            usize::MAX, // Get all available
        );

        // Add to post-accumulation buffer
        post_accumulation_buffer.extend_from_slice(&resampled);

        // Check if we can output
        if !*output_started && post_accumulation_buffer.len() >= target_output_count {
            *output_started = true;
        }

        static POST_ACCUMULATION_RESAMPLING_COUNT: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let post_accumulation_log_count =
            POST_ACCUMULATION_RESAMPLING_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if post_accumulation_log_count % 1000 == 0 {
            info!(
            "🔄 {}: Called resampler with {} input samples, produced {} output samples, current buffer size {}",
            "POST_ACCUMULATION_RESAMPLE_RESULT".cyan(),
            input_samples.len(),
            resampled.len(),
            post_accumulation_buffer.len(),
        );
        }

        // Return chunk if we have enough and started
        if *output_started && post_accumulation_buffer.len() >= target_output_count {
            post_accumulation_buffer
                .drain(0..target_output_count)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Core resampling utility function - performs the actual resampling operation
    fn process_resampling_static(
        resampler: &mut Option<RubatoSRC>,
        input_sample_rate: u32,
        device_sample_rate: u32,
        chunk_size: usize,
        device_id: &str,
        input_samples: &[f32],
        request_count: usize,
    ) -> Vec<f32> {
        if let Some(active_resampler) = Self::get_or_initialize_resampler_static(
            resampler,
            input_sample_rate,
            device_sample_rate,
            chunk_size,
            device_id,
        ) {
            // Convert input samples and get output immediately (stateless)
            active_resampler.convert(input_samples)
        } else {
            // No resampling needed - return original samples or portion
            input_samples[..input_samples.len().min(request_count)].to_vec()
        }
    }

    /// Update adaptive chunk size when input sample rate changes
    fn update_adaptive_chunk_size(
        input_sample_rate: u32,
        device_sample_rate: u32,
        target_chunk_size: usize,
        current_chunk_size: usize,
        device_id: &str,
        #[cfg(target_os = "macos")] hardware_update_tx: &Option<
            mpsc::Sender<crate::audio::mixer::stream_management::AudioCommand>,
        >,
    ) -> usize {
        let optimal_chunk_size =
            calculate_optimal_chunk_size(input_sample_rate, device_sample_rate, target_chunk_size);

        if optimal_chunk_size != current_chunk_size {
            info!("🔧 DYNAMIC_CHUNKS: {} updated chunk size to {} for {}Hz→{}Hz (sample rate changed)",
                  device_id, optimal_chunk_size, input_sample_rate, device_sample_rate);

            // **HARDWARE SYNC**: Update CoreAudio hardware buffer size to match
            #[cfg(target_os = "macos")]
            if let Some(ref hardware_tx) = hardware_update_tx {
                let command = crate::audio::mixer::stream_management::AudioCommand::UpdateOutputHardwareBufferSize {
                    device_id: device_id.to_string(),
                    target_frames: optimal_chunk_size as u32,
                };
                if let Err(e) = hardware_tx.try_send(command) {
                    warn!("⚠️ Failed to send hardware buffer update: {}", e);
                } else {
                    info!(
                        "📡 {}: Sent hardware buffer update to {} frames",
                        "HARDWARE_SYNC_COMMAND".cyan(),
                        optimal_chunk_size
                    );
                }
            }
        }

        optimal_chunk_size
    }

    /// Start the output processing worker thread
    pub fn start(&mut self) -> Result<()> {
        let device_id = self.device_id.clone();
        let device_sample_rate = self.device_sample_rate;
        let target_chunk_size = self.target_chunk_size;

        // Take ownership of receiver and SPMC writer for the worker thread
        let mut mixed_audio_rx =
            std::mem::replace(&mut self.mixed_audio_rx, mpsc::unbounded_channel().1);
        let rtrb_producer = self.rtrb_producer.clone();
        let queue_tracker = self.queue_tracker.clone();

        // Clone hardware update channel for dynamic buffer size updates
        #[cfg(target_os = "macos")]
        let hardware_update_tx = self.hardware_update_tx.clone();

        // Spawn dedicated worker thread
        let worker_handle = tokio::spawn(async move {
            let mut resampler: Option<RubatoSRC> = None;
            let mut chunks_processed = 0u64;
            let mut adaptive_chunk_size = target_chunk_size; // Start with default, adapt on first audio

            // **SAMPLE RATE EQUIVALENCE**: Use direct passthrough when no resampling needed
            let mut needs_resampling = false;

            // **STRATEGY STATE**: State variables for different accumulation strategies
            let mut pre_output_started = false; // Used by pre-accumulation strategy
            let mut post_output_started = false; // Used by post-accumulation strategy

            info!(
                "🚀 OUTPUT_WORKER: Started processing thread for device '{}'",
                device_id
            );

            // **CORE PROCESSING FUNCTIONS**

            /// Core resampling utility function - performs the actual resampling operation
            // **ACCUMULATION BUFFERS**
            let mut pre_accumulation_buffer: Vec<f32> = Vec::new();
            let mut post_accumulation_buffer: Vec<f32> = Vec::new();

            while let Some(mixed_audio) = mixed_audio_rx.recv().await {
                let processing_start = std::time::Instant::now();
                let receive_time = processing_start;

                // **DYNAMIC CHUNK SIZING**: Recalculate chunk size whenever input sample rate changes
                static mut LAST_INPUT_SAMPLE_RATE: Option<u32> = None;
                let input_rate_changed = unsafe {
                    let changed = LAST_INPUT_SAMPLE_RATE
                        .map_or(true, |last_rate| last_rate != mixed_audio.sample_rate);
                    LAST_INPUT_SAMPLE_RATE = Some(mixed_audio.sample_rate);
                    changed
                };

                if input_rate_changed {
                    adaptive_chunk_size = Self::update_adaptive_chunk_size(
                        mixed_audio.sample_rate,
                        device_sample_rate,
                        target_chunk_size,
                        adaptive_chunk_size,
                        &device_id,
                        #[cfg(target_os = "macos")]
                        &hardware_update_tx,
                    );
                }

                // Capture input size before samples are moved
                let input_samples_len = mixed_audio.samples.len();

                // **RESAMPLING DETECTION**: Check if sample rates require resampling
                let sample_rate_difference =
                    (mixed_audio.sample_rate as f32 - device_sample_rate as f32).abs();
                needs_resampling = sample_rate_difference > 1.0; // Allow 1Hz tolerance

                // **TIMING DEBUG**: Track time between receives
                static mut LAST_RECEIVE_TIME: Option<std::time::Instant> = None;
                let time_since_last = unsafe {
                    if let Some(last_time) = LAST_RECEIVE_TIME {
                        Some(receive_time.duration_since(last_time))
                    } else {
                        None
                    }
                };
                unsafe {
                    LAST_RECEIVE_TIME = Some(receive_time);
                }

                // **STEP 1: PROCESS AUDIO BASED ON STRATEGY**
                let resample_start = std::time::Instant::now();
                let rate_ratio = mixed_audio.sample_rate as f32 / device_sample_rate as f32;

                let device_samples = if !needs_resampling {
                    // **NO RESAMPLING**: Direct passthrough when sample rates match
                    mixed_audio.samples.clone()
                } else {
                    if rate_ratio > 1.05 {
                        // **PRE-ACCUMULATION**: For significant downsampling, accumulate input first
                        Self::process_with_pre_accumulation(
                            &mut resampler,
                            mixed_audio.sample_rate,
                            device_sample_rate,
                            adaptive_chunk_size,
                            &device_id,
                            &mixed_audio.samples,
                            adaptive_chunk_size,
                            &mut pre_accumulation_buffer,
                            &rtrb_producer,
                            &mut pre_output_started,
                            Some(&queue_tracker),
                        )
                    } else {
                        // **POST-ACCUMULATION**: For upsampling/minor changes, accumulate output
                        Self::process_with_post_accumulation(
                            &mut resampler,
                            mixed_audio.sample_rate,
                            device_sample_rate,
                            adaptive_chunk_size,
                            &device_id,
                            &mixed_audio.samples,
                            adaptive_chunk_size,
                            &mut post_accumulation_buffer,
                            &mut post_output_started,
                            Some(&queue_tracker),
                        )
                    }
                };

                let resample_duration = resample_start.elapsed();

                // **RESAMPLING PERFORMANCE LOGGING**
                static RESAMPLE_LOG_COUNT: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let resample_count =
                    RESAMPLE_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                if resample_count < 5 || resample_count % 1000 == 0 {
                    let strategy = if !needs_resampling {
                        "DIRECT"
                    } else if rate_ratio > 1.05 {
                        "PRE_ACCUM"
                    } else {
                        "POST_ACCUM"
                    };

                    info!(
                        "🔄 {}: {} strategy: {} → {} samples in {}μs ({}Hz→{}Hz, ratio: {:.3})",
                        "RESAMPLING_STRATEGY".cyan(),
                        strategy,
                        mixed_audio.samples.len(),
                        device_samples.len(),
                        resample_duration.as_micros(),
                        mixed_audio.sample_rate,
                        device_sample_rate,
                        rate_ratio
                    );
                }

                // **QUEUE STATE LOGGING**: Log queue occupancy every 1000th call
                static QUEUE_LOG_COUNT: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let queue_log_count =
                    QUEUE_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                if queue_log_count % 1000 == 0 {
                    let queue_info = queue_tracker.get_queue_info();
                    info!(
                        "📊 {}: occupancy={:.1}% ({}/{}) adjustment_ratio={} integral_error={} target_fill={} device={}",
                        "QUEUE_STATE".green(),
                        queue_info.usage_percent,
                        queue_info.estimated_occupancy,
                        queue_info.capacity,
                        queue_info.ratio,
                        queue_info.integral_error,
                        queue_info.target_fill,
                        device_id
                    );
                }

                // **OPTIMIZATION**: If no resampling and chunk size matches, bypass accumulation entirely
                let mut chunks_sent_this_cycle = 0;
                let mut total_rtrb_duration = std::time::Duration::ZERO;

                // **STEP 2: SEND PROCESSED SAMPLES TO RTRB QUEUE**

                if !device_samples.is_empty() {
                    let rtrb_write_start = std::time::Instant::now();
                    if let Some(ref rtrb_producer) = rtrb_producer {
                        Self::write_to_hardware_rtrb(
                            &device_id,
                            &device_samples,
                            rtrb_producer,
                            Some(&queue_tracker),
                        )
                        .await;
                    }
                    let rtrb_write_duration = rtrb_write_start.elapsed();
                    total_rtrb_duration += rtrb_write_duration;

                    chunks_processed += 1;
                    chunks_sent_this_cycle += 1;

                    // Rate-limited logging for strategy output
                    if chunks_processed <= 5 || chunks_processed % 1000 == 0 {
                        let strategy_label = if !needs_resampling {
                            "🔄DIRECT"
                        } else if rate_ratio > 1.05 {
                            "🔄PRE_ACCUM"
                        } else {
                            "🔄POST_ACCUM"
                        };

                        info!(
                            "🎵 {} (4th layer): {} sent chunk #{} ({} samples) {}",
                            "OUTPUT_WORKER".purple(),
                            device_id,
                            chunks_processed,
                            device_samples.len(),
                            strategy_label
                        );
                    }
                }

                let processing_duration = processing_start.elapsed();

                // **COMPREHENSIVE TIMING DIAGNOSTICS** for downsampling stuttering
                static TIMING_DEBUG_COUNT: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let debug_count =
                    TIMING_DEBUG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                if (debug_count < 10 || debug_count % 1000 == 0) {
                    let time_between = if let Some(gap) = time_since_last {
                        format!("{}μs", gap.as_micros())
                    } else {
                        "N/A".to_string()
                    };

                    info!("⏱️  {} [{}]: gap_since_last={}, input={}→{} samples, 🔄resample={}μs, chunks_sent={}, rtrb={}μs, total={}μs (FFT_FIXED_IN)",
                        "OUTPUT_TIMING".purple(),
                        device_id,
                        time_between,
                        input_samples_len,
                        device_samples.len(),
                        resample_duration.as_micros(),
                        chunks_sent_this_cycle,
                        total_rtrb_duration.as_micros(),
                        processing_duration.as_micros()
                    );
                }

                // Performance monitoring
                use std::sync::atomic::{AtomicU64, Ordering};
                static OUTPUT_WORKER_COUNT: AtomicU64 = AtomicU64::new(0);
                let count = OUTPUT_WORKER_COUNT.fetch_add(1, Ordering::Relaxed);
                if processing_duration.as_micros() > 500 && (count <= 3 || count % 1000 == 0) {
                    warn!(
                        "🐌 {}: {} SLOW processing: {}μs (🔄resample: {}μs, rtrb: {}μs) [FFT_FIXED_IN]",
                        "OUTPUT_WORKER_SLOW".bright_red(),
                        device_id,
                        processing_duration.as_micros(),
                        resample_duration.as_micros(),
                        total_rtrb_duration.as_micros()
                    );
                }
            }

            info!(
                "🛑 OUTPUT_WORKER: Processing thread for '{}' shutting down (processed {} chunks)",
                device_id, chunks_processed
            );
        });

        self.worker_handle = Some(worker_handle);
        info!(
            "✅ OUTPUT_WORKER: Started worker thread for device '{}'",
            self.device_id
        );

        Ok(())
    }

    /// Stop the output processing worker
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();

            match tokio::time::timeout(std::time::Duration::from_millis(100), handle).await {
                Ok(_) => info!(
                    "✅ OUTPUT_WORKER: '{}' shut down gracefully",
                    self.device_id
                ),
                Err(_) => warn!(
                    "⚠️ OUTPUT_WORKER: '{}' force-terminated after timeout",
                    self.device_id
                ),
            }
        }

        Ok(())
    }

    /// Get RTRB queue occupancy information

    /// Sync helper to write samples to RTRB queue with tracking
    fn write_samples_to_rtrb_sync(
        device_id: &str,
        samples: &[f32],
        rtrb_producer: &Arc<Mutex<Producer<f32>>>,
        queue_tracker: Option<&AtomicQueueTracker>,
    ) {
        let lock_start = std::time::Instant::now();
        if let Ok(mut producer) = rtrb_producer.try_lock() {
            let lock_duration = lock_start.elapsed();

            // **RTRB BULK WRITE**: Use efficient chunk writing
            let mut samples_written = 0;
            let mut remaining = samples;

            while !remaining.is_empty() && samples_written < samples.len() {
                let chunk_size = remaining.len().min(producer.slots());
                if chunk_size == 0 {
                    // Ring buffer is full, drop remaining samples
                    // warn!(
                    //     "⚠️ OUTPUT_WORKER: {} RTRB queue full, dropping {} remaining samples",
                    //     device_id,
                    //     remaining.len()
                    // );
                    break;
                }

                let chunk = &remaining[..chunk_size];
                for &sample in chunk {
                    if producer.push(sample).is_err() {
                        break;
                    }
                    samples_written += 1;
                }
                remaining = &remaining[chunk_size..];
            }

            // Record samples written for queue tracking
            if let Some(tracker) = queue_tracker {
                tracker.record_samples_written(samples_written);
            }

            // Queue tracking is now handled by the AtomicQueueTracker
        } else {
            warn!(
                "⚠️ OUTPUT_WORKER: {} failed to lock RTRB producer, dropping {} samples",
                device_id,
                samples.len()
            );
        }
    }

    /// Write audio samples to hardware via RTRB queue (async wrapper)
    async fn write_to_hardware_rtrb(
        device_id: &str,
        samples: &[f32],
        rtrb_producer: &Arc<Mutex<Producer<f32>>>,
        queue_tracker: Option<&AtomicQueueTracker>,
    ) {
        Self::write_samples_to_rtrb_sync(device_id, samples, rtrb_producer, queue_tracker);
    }

    /// Adjusts the active resampler's output rate to correct drift.
    ///
    /// # Arguments
    /// - `fill`: current queue fill (frames/samples).
    /// - `capacity`: total queue capacity.
    /// - `in_rate`: input sample rate (Hz).
    /// - `out_rate_nom`: nominal output sample rate (Hz).
    fn adjust_dynamic_sample_rate_static(
        resampler: &mut Option<RubatoSRC>,
        queue_tracker: Option<&AtomicQueueTracker>,
        input_sample_rate: u32,
        device_sample_rate: u32,
        chunk_size: usize,
        device_id: &str,
    ) {
        let can_adjust_dynamically = if let Some(active_resampler) =
            Self::get_or_initialize_resampler_static(
                resampler,
                input_sample_rate,
                device_sample_rate,
                chunk_size,
                device_id,
            ) {
            active_resampler.supports_dynamic_sample_rate()
        } else {
            // Fallback to manual calculation if no resampling needed
            false
        };

        if !can_adjust_dynamically {
            println!("adjusting dynamically not supported");
            return;
        }

        // Get queue info from tracker if available, otherwise skip adjustment
        let adjusted_ratio = if let Some(tracker) = queue_tracker {
            tracker.adjust_ratio(input_sample_rate, device_sample_rate)
        } else {
            // No queue tracker available - can't do dynamic adjustment
            trace!(
                "🎯 {}: No queue tracker available for {} - skipping dynamic adjustment",
                "DYNAMIC_RATE".yellow(),
                device_id
            );
            return;
        };

        let new_out_rate = input_sample_rate as f32 * adjusted_ratio;

        if let Some(active_resampler) = Self::get_or_initialize_resampler_static(
            resampler,
            input_sample_rate,
            device_sample_rate,
            chunk_size,
            device_id,
        ) {
            if let Err(err) =
                active_resampler.set_sample_rates(input_sample_rate as f32, new_out_rate, true)
            {
                warn!("⚠️ Drift correction failed: {}", err);
            }
        }
    }

    /// Get processing statistics
    pub fn get_stats(&self) -> OutputWorkerStats {
        OutputWorkerStats {
            device_id: self.device_id.clone(),
            chunks_processed: self.chunks_processed,
            samples_output: self.samples_output,
            buffer_size: self.sample_buffer.len(),
            target_chunk_size: self.target_chunk_size,
            is_running: self.worker_handle.is_some(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueueInfo {
    pub occupancy: usize,
    pub capacity: usize,
    pub usage_percent: f32,
    pub available: usize,
}

#[derive(Debug, Clone)]
pub struct OutputWorkerStats {
    pub device_id: String,
    pub chunks_processed: u64,
    pub samples_output: u64,
    pub buffer_size: usize,
    pub target_chunk_size: usize,
    pub is_running: bool,
}
