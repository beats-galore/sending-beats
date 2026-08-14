// Fixed-size block accumulation for the mixing layer
//
// Input devices deliver on independent hardware clocks with different callback
// sizes and intervals. Rather than trying to align those deliveries against each
// other, every device's audio is accumulated into a flat per-device queue and the
// mixer consumes a fixed-size block from each one per cycle.
//
// This gives the mix a single stable cadence: a device that is behind contributes
// silence for the remainder of the block instead of stalling the mix, and a device
// that is ahead simply keeps its surplus for the next block.

use colored::*;
use std::collections::{HashMap, VecDeque};
use tracing::warn;

/// Per-device sample accumulation feeding fixed-size mix blocks
#[derive(Debug)]
pub struct BlockAccumulator {
    /// Flat sample queue per device, already converted to the mix channel layout
    device_samples: HashMap<String, VecDeque<f32>>,
    /// Number of samples emitted per device per block
    block_samples: usize,
    /// Maximum backlog per device before the oldest audio is discarded
    max_samples: usize,
}

impl BlockAccumulator {
    /// # Arguments
    /// * `block_samples` - samples emitted per device per mix block
    /// * `max_blocks` - backlog allowed per device before dropping oldest audio
    pub fn new(block_samples: usize, max_blocks: usize) -> Self {
        Self {
            device_samples: HashMap::new(),
            block_samples,
            max_samples: block_samples * max_blocks,
        }
    }

    /// Append freshly collected samples for a device
    pub fn push(&mut self, device_id: &str, samples: &[f32]) {
        let queue = self
            .device_samples
            .entry(device_id.to_string())
            .or_default();

        queue.extend(samples.iter().copied());

        // Backpressure should keep this from triggering. If it does, the device is
        // producing faster than the output consumes and the oldest audio is the
        // least useful to keep.
        if queue.len() > self.max_samples {
            let excess = queue.len() - self.max_samples;
            queue.drain(..excess);

            static OVERFLOW_LOG: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let log_count = OVERFLOW_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if log_count % 100 == 0 {
                warn!(
                    "⚠️ {}: Device '{}' backlog exceeded {} samples, dropped {} oldest",
                    "BLOCK_ACCUMULATOR".on_yellow().green(),
                    device_id,
                    self.max_samples,
                    excess
                );
            }
        }
    }

    /// Drop a device and any audio it still had buffered
    pub fn remove_device(&mut self, device_id: &str) {
        self.device_samples.remove(device_id);
    }

    /// How much audio a device is still holding, in samples
    ///
    /// Everything past the first block is delay: it was captured but has to wait
    /// for as many cycles as it takes to drain ahead of it.
    pub fn backlog_samples(&self, device_id: &str) -> usize {
        self.device_samples
            .get(device_id)
            .map_or(0, |queue| queue.len())
    }

    /// Take one block of exactly `block_samples` from every device holding audio
    ///
    /// Devices with no audio at all are omitted, contributing nothing to the mix.
    /// A device holding a partial block is padded with silence so every returned
    /// block is the same length and the mix advances by exactly one block.
    ///
    /// Returns `None` when no device has any audio.
    pub fn take_block(&mut self) -> Option<Vec<(String, Vec<f32>)>> {
        let mut blocks: Vec<(String, Vec<f32>)> = Vec::new();

        for (device_id, queue) in self.device_samples.iter_mut() {
            if queue.is_empty() {
                continue;
            }

            let take = queue.len().min(self.block_samples);
            let mut block: Vec<f32> = queue.drain(..take).collect();
            block.resize(self.block_samples, 0.0);

            blocks.push((device_id.clone(), block));
        }

        if blocks.is_empty() {
            None
        } else {
            Some(blocks)
        }
    }
}
