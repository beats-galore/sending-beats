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
//
// A device is only ever *one* block behind, though. The mixer is paced by the
// output hardware, so it consumes at exactly realtime and never faster, and a
// queue holding more than one delivery has no way to work the surplus off — every
// sample of it is delay for the rest of the session. So anything beyond what the
// device delivers at a time is shed rather than carried.

use colored::*;
use std::collections::HashMap;
use std::collections::VecDeque;
use tracing::warn;

/// What one device is holding, and how much it tends to arrive with
#[derive(Debug, Default)]
struct DeviceBacklog {
    /// Samples waiting, already converted to the mix channel layout
    samples: VecDeque<f32>,
    /// Largest of the last two deliveries
    ///
    /// A device cannot be trimmed below what it hands over at a time or its
    /// audio would be cut apart on arrival — a ScreenCaptureKit tap delivers 960
    /// frames at once and legitimately needs to hold all of them. Tracking only
    /// the last two means a one-off surge, from the mixer being descheduled and
    /// draining a full queue at once, stops counting almost immediately.
    last_delivery: usize,
    previous_delivery: usize,
}

impl DeviceBacklog {
    fn record_delivery(&mut self, samples: usize) {
        self.previous_delivery = self.last_delivery;
        self.last_delivery = samples;
    }

    /// Most this device should be holding once the mixer has taken its block
    ///
    /// One delivery, because a device cannot be trimmed below what it hands over
    /// at a time without its audio being cut apart on arrival, plus a cushion for
    /// the mixer and this device drifting against each other. The mixer pads a
    /// device short of a full block with silence rather than waiting, so running
    /// with no cushion turns ordinary jitter into a gap every block.
    fn steady_state(&self, block_samples: usize, cushion_samples: usize) -> usize {
        self.last_delivery
            .max(self.previous_delivery)
            .max(block_samples)
            + cushion_samples
    }
}

/// Per-device sample accumulation feeding fixed-size mix blocks
#[derive(Debug)]
pub struct BlockAccumulator {
    device_samples: HashMap<String, DeviceBacklog>,
    /// Number of samples emitted per device per block
    block_samples: usize,
    /// Held on top of a delivery so ordinary drift does not become a silence gap
    cushion_samples: usize,
    /// Hard ceiling per device, whatever its delivery size
    max_samples: usize,
}

impl BlockAccumulator {
    /// # Arguments
    /// * `block_samples` - samples emitted per device per mix block
    /// * `cushion_samples` - held on top of a delivery to absorb drift
    /// * `max_samples` - absolute ceiling per device, however it delivers
    ///
    /// The ceiling is in samples rather than blocks on purpose: it is a last
    /// resort against a device that delivers pathologically, and scaling it with
    /// the block would shrink it as blocks get smaller. In normal running it is
    /// never reached, because [`take_block`](Self::take_block) sheds down to a
    /// delivery plus the cushion.
    pub fn new(block_samples: usize, cushion_samples: usize, max_samples: usize) -> Self {
        Self {
            device_samples: HashMap::new(),
            block_samples,
            cushion_samples,
            max_samples,
        }
    }

    /// Resize the drift cushion, which follows the pipeline's sample rate
    pub fn set_cushion_samples(&mut self, cushion_samples: usize) {
        self.cushion_samples = cushion_samples;
    }

    /// Resize the block the mixer consumes, so it can follow the output hardware
    ///
    /// Whatever devices are already holding stays put, and comes out at the new
    /// size from the next block onward.
    pub fn set_block_samples(&mut self, block_samples: usize) {
        self.block_samples = block_samples;
    }

    pub fn block_samples(&self) -> usize {
        self.block_samples
    }

    /// Append freshly collected samples for a device
    pub fn push(&mut self, device_id: &str, samples: &[f32]) {
        let backlog = self
            .device_samples
            .entry(device_id.to_string())
            .or_default();

        backlog.samples.extend(samples.iter().copied());
        backlog.record_delivery(samples.len());

        // Shedding on take keeps this from being reached. A device this far ahead
        // is delivering faster than the mix consumes, and the oldest audio is the
        // least useful to keep.
        if backlog.samples.len() > self.max_samples {
            let excess = backlog.samples.len() - self.max_samples;
            backlog.samples.drain(..excess);

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
            .map_or(0, |backlog| backlog.samples.len())
    }

    /// Take one block of exactly `block_samples` from every device holding audio
    ///
    /// Devices with no audio at all are omitted, contributing nothing to the mix.
    /// A device holding a partial block is padded with silence so every returned
    /// block is the same length and the mix advances by exactly one block.
    ///
    /// Anything a device still holds beyond one delivery afterwards is dropped.
    /// The mixer runs at exactly the output's rate and can never catch up by
    /// consuming faster, so a surplus left in place is not a buffer — it is delay
    /// on every sample behind it, for as long as the device stays connected. One
    /// audible discontinuity now costs less than permanent latency.
    ///
    /// Returns `None` when no device has any audio.
    pub fn take_block(&mut self) -> Option<Vec<(String, Vec<f32>)>> {
        let mut blocks: Vec<(String, Vec<f32>)> = Vec::new();

        for (device_id, backlog) in self.device_samples.iter_mut() {
            if backlog.samples.is_empty() {
                continue;
            }

            let take = backlog.samples.len().min(self.block_samples);
            let mut block: Vec<f32> = backlog.samples.drain(..take).collect();
            block.resize(self.block_samples, 0.0);

            let steady_state = backlog.steady_state(self.block_samples, self.cushion_samples);
            if backlog.samples.len() > steady_state {
                let shed = backlog.samples.len() - steady_state;
                backlog.samples.drain(..shed);

                static SHED_LOG: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let log_count = SHED_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if log_count % 100 == 0 {
                    warn!(
                        "⚠️ {}: Device '{}' was {} samples behind, shed to {} (occurrence #{})",
                        "BLOCK_ACCUMULATOR".on_yellow().green(),
                        device_id,
                        shed,
                        steady_state,
                        log_count
                    );
                }
            }

            blocks.push((device_id.clone(), block));
        }

        if blocks.is_empty() {
            None
        } else {
            Some(blocks)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: usize = 256;
    const CUSHION: usize = 960;
    const CEILING: usize = 8192;

    fn accumulator() -> BlockAccumulator {
        BlockAccumulator::new(BLOCK, CUSHION, CEILING)
    }

    #[test]
    fn a_device_delivering_one_block_at_a_time_holds_nothing_extra() {
        let mut accumulator = accumulator();

        for _ in 0..10 {
            accumulator.push("mic", &vec![0.5; BLOCK]);
            accumulator.take_block();
        }

        assert_eq!(accumulator.backlog_samples("mic"), 0);
    }

    #[test]
    fn a_surplus_is_shed_rather_than_carried_forever() {
        let mut accumulator = accumulator();

        // A descheduled mixer drains a whole queue at once when it resumes
        accumulator.push("mic", &vec![0.5; BLOCK * 30]);
        accumulator.take_block();
        assert_eq!(accumulator.backlog_samples("mic"), BLOCK * 29);

        for _ in 0..3 {
            accumulator.push("mic", &vec![0.5; BLOCK]);
            accumulator.take_block();
        }

        // Down to a delivery plus the cushion, not the twenty-nine blocks it would
        // otherwise carry as delay on everything behind them for the whole session
        assert_eq!(accumulator.backlog_samples("mic"), BLOCK + CUSHION);
    }

    #[test]
    fn the_cushion_is_kept_rather_than_shed() {
        let mut accumulator = accumulator();

        // Running a block ahead is what stops ordinary drift becoming a silence
        // gap, so it must survive a take rather than being trimmed as surplus.
        accumulator.push("mic", &vec![0.5; BLOCK * 2]);
        accumulator.take_block();

        assert_eq!(accumulator.backlog_samples("mic"), BLOCK);
    }

    #[test]
    fn a_coarse_source_keeps_the_whole_delivery() {
        let mut accumulator = accumulator();

        // A ScreenCaptureKit tap hands over 960 frames at once, seven blocks
        let delivery = 960 * 2;
        accumulator.push("music", &vec![0.5; delivery]);
        accumulator.take_block();

        // Shedding to the block size here would cut every delivery apart on arrival
        assert_eq!(accumulator.backlog_samples("music"), delivery - BLOCK);
    }

    #[test]
    fn a_coarse_source_is_still_bounded_at_one_delivery() {
        let mut accumulator = accumulator();
        let delivery = 960 * 2;

        // Three deliveries arrive before the mixer takes anything
        for _ in 0..3 {
            accumulator.push("music", &vec![0.5; delivery]);
        }
        accumulator.take_block();

        assert_eq!(accumulator.backlog_samples("music"), delivery + CUSHION);
    }

    #[test]
    fn a_partial_block_is_padded_not_stretched() {
        let mut accumulator = accumulator();
        accumulator.push("mic", &vec![0.5; BLOCK / 2]);

        let blocks = accumulator.take_block().expect("a block");
        let (_, block) = &blocks[0];

        assert_eq!(block.len(), BLOCK);
        assert_eq!(block[BLOCK / 2], 0.0);
    }
}
