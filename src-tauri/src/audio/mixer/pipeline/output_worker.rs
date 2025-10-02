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
            "🔊 {}: Creating worker for device '{}' ({} Hz, {} sample chunks, hardware: {})",
            "OUTPUT_WORKER".on_blue().yellow(),
            device_id,
            device_sample_rate,
            target_chunk_size,
            has_hardware_output
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
            "🔊 {}: Creating worker with hardware updates for device '{}' ({} Hz, {} sample chunks, hardware: {})",
            "OUTPUT_WORKER".on_blue().yellow(),
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
                            "DYNAMIC_RATE_UPDATE".on_blue().yellow(),
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
                            "DYNAMIC_RATE_FAILED".on_blue().yellow(),
                            self.device_id,
                            e
                        );
                        // Fall through to recreation
                    }
                }
            } else {
                info!(
                    "🔄 {}: Resampler for {} doesn't support dynamic rates, recreating",
                    "RESAMPLER_RECREATION".on_blue().yellow(),
                    self.device_id
                );
            }
        }

        // Fallback: force resampler recreation on next processing cycle
        self.resampler = None;
        info!(
            "🔧 {}: Marked resampler for recreation - new mix rate: {}Hz",
            "RESAMPLER_RESET".on_blue().yellow(),
            target_mix_rate
        );
        Ok(())
    }

    /// Get queue tracker for sharing with CoreAudio callback
    pub fn get_queue_tracker_for_consumer(&self) -> AtomicQueueTracker {
        self.queue_tracker.clone()
    }

    /// Static helper function to get or initialize resampler in async context
    /// Since we use SincFixedOut, we can dynamically adjust rates without recreation
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

        // If resampler exists, adjust it dynamically; otherwise create new one
        match resampler {
            Some(ref mut existing_resampler) => {
                let rates_changed = existing_resampler.input_rate() != input_sample_rate
                    || existing_resampler.output_rate() != output_sample_rate;

                if rates_changed {
                    // Use dynamic adjustment - SincFixedOut supports this
                    if let Err(e) = existing_resampler.set_sample_rates(
                        input_sample_rate as f32,
                        output_sample_rate as f32,
                        true, // Use ramping for smooth transitions
                    ) {
                        warn!(
                            "⚠️ {}: Dynamic adjustment failed for {}: {} - recreating resampler",
                            "DYNAMIC_ADJUST_FAILED".on_blue().yellow(),
                            device_id,
                            e
                        );
                        // If dynamic adjustment fails, create a new resampler
                        *resampler = None;
                    }
                }
            }
            None => {
                // Create new resampler
                match RubatoSRC::new_sinc_fixed_output(
                    input_sample_rate as f32,
                    output_sample_rate as f32,
                    chunk_size / 2,
                    2,
                    format!("output_{}", device_id), // Identifier for logging
                ) {
                    Ok(new_resampler) => {
                        info!(
                            "🔄 {}: Created resampler for {} ({} Hz → {} Hz, ratio: {:.3})",
                            "OUTPUT_RESAMPLER".on_blue().yellow(),
                            device_id,
                            input_sample_rate,
                            output_sample_rate,
                            new_resampler.ratio()
                        );
                        *resampler = Some(new_resampler);
                    }
                    Err(e) => {
                        error!(
                            "❌ {}: Failed to create resampler for {}: {}",
                            "OUTPUT_WORKER".on_blue().yellow(),
                            device_id,
                            e
                        );
                        return None;
                    }
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
                "BUFFER_DRIFT_TRACK".on_blue().yellow(),
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

                    // Apply dynamic rate adjustment using shared function
                    if let Some(active_resampler) = resampler.as_mut() {
                        if let Some(tracker) = queue_tracker {
                            use crate::audio::mixer::pipeline::resampling_accumulator;
                            let _ = resampling_accumulator::adjust_dynamic_sample_rate(
                                active_resampler,
                                tracker,
                                input_sample_rate,
                                device_sample_rate,
                                device_id,
                            );
                        }
                    }
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
            "POST_ACCUMULATION_RESAMPLE_RESULT".on_blue().yellow(),
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
                        "HARDWARE_SYNC_COMMAND".on_blue().yellow(),
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
                "🚀 {}: Started processing thread for device '{}'",
                "OUTPUT_WORKER".on_blue().yellow(),
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
                if needs_resampling {
                    info!(
                        "🔄 {}: Sample rate difference for {} - {}Hz→{}Hz (ratio: {:.3})",
                        "RESAMPLING_DETECTED".on_blue().yellow(),
                        device_id,
                        mixed_audio.sample_rate,
                        device_sample_rate,
                        sample_rate_difference
                    );
                }

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
                        "RESAMPLING_STRATEGY".on_blue().yellow(),
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
                        "QUEUE_STATE".on_blue().yellow(),
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
                        Self::write_samples_to_rtrb_sync(
                            &device_id,
                            &device_samples,
                            rtrb_producer,
                            Some(&queue_tracker),
                        );
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
                            "OUTPUT_WORKER".on_blue().yellow(),
                            device_id,
                            chunks_processed,
                            device_samples.len(),
                            strategy_label
                        );
                    }
                }

                let processing_duration = processing_start.elapsed();

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
                        "OUTPUT_TIMING".on_blue().yellow(),
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

                use std::sync::atomic::{AtomicU64, Ordering};
                static OUTPUT_WORKER_COUNT: AtomicU64 = AtomicU64::new(0);
                let count = OUTPUT_WORKER_COUNT.fetch_add(1, Ordering::Relaxed);
                if processing_duration.as_micros() > 500 && (count <= 3 || count % 1000 == 0) {
                    warn!(
                        "🐌 {}: {} SLOW processing: {}μs (🔄resample: {}μs, rtrb: {}μs) [FFT_FIXED_IN]",
                        "OUTPUT_WORKER_SLOW".on_blue().yellow(),
                        device_id,
                        processing_duration.as_micros(),
                        resample_duration.as_micros(),
                        total_rtrb_duration.as_micros()
                    );
                }
            }

            info!(
                "🛑 {}: Processing thread for '{}' shutting down (processed {} chunks)",
                "OUTPUT_WORKER".on_blue().yellow(),
                device_id,
                chunks_processed
            );
        });

        self.worker_handle = Some(worker_handle);
        info!(
            "✅ {}: Started worker thread for device '{}'",
            "OUTPUT_WORKER".on_blue().yellow(),
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
                    "✅ {}: '{}' shut down gracefully",
                    "OUTPUT_WORKER".on_blue().yellow(),
                    self.device_id
                ),
                Err(_) => warn!(
                    "⚠️ {}: '{}' force-terminated after timeout",
                    "OUTPUT_WORKER".on_blue().yellow(),
                    self.device_id
                ),
            }
        }

        Ok(())
    }

    fn write_samples_to_rtrb_sync(
        device_id: &str,
        samples: &[f32],
        rtrb_producer: &Arc<Mutex<Producer<f32>>>,
        queue_tracker: Option<&AtomicQueueTracker>,
    ) {
        let lock_start = std::time::Instant::now();
        if let Ok(mut producer) = rtrb_producer.try_lock() {
            let lock_duration = lock_start.elapsed();

            let mut samples_written = 0;
            let mut remaining = samples;

            while !remaining.is_empty() && samples_written < samples.len() {
                let chunk_size = remaining.len().min(producer.slots());
                if chunk_size == 0 {
                    // Ring buffer is full, drop remaining samples
                    warn!(
                        "⚠️ OUTPUT_WORKER: {} RTRB queue full, dropping {} remaining samples",
                        device_id,
                        remaining.len()
                    );
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
        } else {
            warn!(
                "⚠️ {}: {} failed to lock RTRB producer, dropping {} samples",
                "OUTPUT_WORKER".on_blue().yellow(),
                device_id,
                samples.len()
            );
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
