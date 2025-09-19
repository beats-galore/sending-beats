// Audio utility functions for shared logic across the pipeline

use tracing::info;
use colored::*;

/// Calculate optimal chunk size for downsampling to avoid waiting cycles
/// This ensures consistent audio flow by reducing chunk sizes when downsampling occurs
pub fn calculate_optimal_chunk_size(
    input_sample_rate: u32,
    output_sample_rate: u32,
    base_chunk_size: usize,
) -> usize {
    let rate_ratio = input_sample_rate as f32 / output_sample_rate as f32;

    // Only adjust for downsampling (rate_ratio > 1.0)
    if rate_ratio > 1.05 {
        // Reduce chunk size to next power of 2 down to ensure we get samples every cycle
        let optimal_size = if base_chunk_size >= 1024 {
            1024 // 1024 → 512 for typical downsampling
        } else if base_chunk_size >= 512 {
            512 // 512 → 256
        } else {
            base_chunk_size // Keep small sizes as-is
        };

        info!(
            "🎯 {}: Downsampling {}Hz→{}Hz (ratio: {:.3}), chunk {} → {} for consistent flow",
            "CHUNK_OPTIMIZATION".purple(),
            input_sample_rate,
            output_sample_rate,
            rate_ratio,
            base_chunk_size,
            optimal_size
        );
        optimal_size
    } else {
        // No downsampling - keep original size
        base_chunk_size
    }
}