// Shared resampling accumulation utilities for input and output workers
use crate::audio::mixer::queue_manager::AtomicQueueTracker;
use crate::audio::mixer::resampling::rubato::RubatoSRC;
use colored::Colorize;
use tracing::{info, trace, warn};

/// Pre-accumulation strategy: Collect enough input samples before resampling
/// Used for upsampling: accumulate input frames until we have enough to produce target output
pub fn process_with_pre_accumulation(
    resampler: &mut RubatoSRC,
    input_samples: &[f32],
    accumulation_buffer: &mut Vec<f32>,
    target_output_samples: usize,
) -> Option<Vec<f32>> {
    // Step 1: Add incoming samples to accumulation buffer
    accumulation_buffer.extend_from_slice(input_samples);

    // Step 2: Check if we have enough input to produce target output
    let input_frames_needed = resampler.input_frames_needed(target_output_samples / 2) * 2;

    // **BUFFER MONITORING**: Track accumulation levels
    static BUFFER_LEVEL_LOG_COUNT: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0);
    let buffer_log_count =
        BUFFER_LEVEL_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if buffer_log_count % 1000 == 0 {
        info!(
            "🔄 PRE_ACCUM: Buffer {} samples, need {} for output",
            accumulation_buffer.len(),
            input_frames_needed
        );
    }

    // Step 3: If we have enough, resample and drain the used input
    if accumulation_buffer.len() >= input_frames_needed {
        let resampled = resampler.convert(&accumulation_buffer[..input_frames_needed]);

        // Drain the used input samples
        accumulation_buffer.drain(..input_frames_needed);

        Some(resampled)
    } else {
        // Not enough input yet, keep accumulating
        None
    }
}

/// Dynamic sample rate adjustment using queue tracker to prevent drift
/// Monitors queue fill levels and adjusts resampling ratio accordingly
pub fn adjust_dynamic_sample_rate(
    resampler: &mut RubatoSRC,
    queue_tracker: &AtomicQueueTracker,
    input_sample_rate: u32,
    device_sample_rate: u32,
    device_id: &str,
) -> Result<(), String> {
    if !resampler.supports_dynamic_sample_rate() {
        trace!(
            "🎯 {}: Resampler for {} does not support dynamic rate adjustment",
            "DYNAMIC_RATE".yellow(),
            device_id
        );
        return Ok(());
    }

    // Get adjusted ratio from queue tracker
    let adjusted_ratio = queue_tracker.adjust_ratio(input_sample_rate, device_sample_rate);
    let new_out_rate = input_sample_rate as f32 * adjusted_ratio;

    // Apply the adjusted sample rate
    resampler
        .set_sample_rates(input_sample_rate as f32, new_out_rate, true)
        .map_err(|err| {
            warn!("⚠️ Drift correction failed: {}", err);
            err.to_string()
        })
}
