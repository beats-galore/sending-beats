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
// Being ahead is not the same as being late, though. The mixer is paced by the
// output hardware, so it consumes at exactly realtime and never faster, and audio
// a device is holding that never drains cannot be worked off — it stays in front
// of everything behind it for the rest of the session. A queue that fills and
// empties is doing its job; one with a floor it never reaches under is carrying
// delay, and that floor is what gets shed.

use colored::*;
use std::collections::HashMap;
use std::collections::VecDeque;
use tracing::warn;

/// What one device is holding, and how far down it has drained lately
#[derive(Debug)]
struct DeviceBacklog {
    /// Samples waiting, already converted to the mix channel layout
    samples: VecDeque<f32>,
    /// Least it has held at any point in the current window
    ///
    /// This is what separates a burst from a surplus. Sources deliver on their
    /// own schedules — a ScreenCaptureKit tap arrives in batches of several
    /// callbacks at once and then goes quiet — and audio that arrives early is
    /// played on time as long as it drains again. Only a level the queue never
    /// falls below is delay, because nothing behind it can move up.
    window_floor: usize,
    /// Blocks taken since the floor was last acted on
    blocks_this_window: usize,
}

impl Default for DeviceBacklog {
    fn default() -> Self {
        Self {
            samples: VecDeque::new(),
            window_floor: usize::MAX,
            blocks_this_window: 0,
        }
    }
}

/// Per-device sample accumulation feeding fixed-size mix blocks
#[derive(Debug)]
pub struct BlockAccumulator {
    device_samples: HashMap<String, DeviceBacklog>,
    /// Number of samples emitted per device per block
    block_samples: usize,
    /// Kept in hand so ordinary drift does not become a silence gap
    cushion_samples: usize,
    /// Audio spanned by one window of observation before a floor is acted on
    window_samples: usize,
    /// Hard ceiling per device, whatever its delivery pattern
    max_samples: usize,
}

impl BlockAccumulator {
    /// # Arguments
    /// * `block_samples` - samples emitted per device per mix block
    /// * `cushion_samples` - kept in hand to absorb drift
    /// * `window_samples` - audio observed before a standing floor is acted on
    /// * `max_samples` - absolute ceiling per device, however it delivers
    ///
    /// The ceiling is in samples rather than blocks on purpose: it is a last
    /// resort against a device that delivers pathologically, and scaling it with
    /// the block would shrink it as blocks get smaller.
    pub fn new(
        block_samples: usize,
        cushion_samples: usize,
        window_samples: usize,
        max_samples: usize,
    ) -> Self {
        Self {
            device_samples: HashMap::new(),
            block_samples,
            cushion_samples,
            window_samples,
            max_samples,
        }
    }

    /// Resize the drift cushion, which follows the pipeline's sample rate
    pub fn set_cushion_samples(&mut self, cushion_samples: usize) {
        self.cushion_samples = cushion_samples;
    }

    /// Blocks spanned by one window of observation
    ///
    /// Has to be comfortably longer than the gap between a source's deliveries,
    /// or a burst arriving on the last block of a window looks like a floor that
    /// was never drained.
    fn window_blocks(&self) -> usize {
        (self.window_samples / self.block_samples.max(1)).max(1)
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
    /// Once a window has passed, whatever a device never drained below is shed.
    /// The mixer runs at exactly the output's rate and can never catch up by
    /// consuming faster, so a floor left in place is not a buffer — it is delay on
    /// every sample behind it for as long as the device stays connected. Bursts
    /// are left alone: arriving early costs nothing as long as it drains again.
    ///
    /// Returns `None` when no device has any audio.
    pub fn take_block(&mut self) -> Option<Vec<(String, Vec<f32>)>> {
        let mut blocks: Vec<(String, Vec<f32>)> = Vec::new();
        let window_blocks = self.window_blocks();

        for (device_id, backlog) in self.device_samples.iter_mut() {
            if backlog.samples.is_empty() {
                continue;
            }

            let take = backlog.samples.len().min(self.block_samples);
            let mut block: Vec<f32> = backlog.samples.drain(..take).collect();
            block.resize(self.block_samples, 0.0);

            backlog.window_floor = backlog.window_floor.min(backlog.samples.len());
            backlog.blocks_this_window += 1;

            if backlog.blocks_this_window >= window_blocks {
                // Whatever the queue never got below over a whole window is audio
                // no arrival pattern accounts for. It is not buffering anything,
                // it is sitting in front of everything behind it.
                if backlog.window_floor > self.cushion_samples {
                    let shed = backlog.window_floor - self.cushion_samples;
                    backlog.samples.drain(..shed.min(backlog.samples.len()));

                    static SHED_LOG: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(0);
                    let log_count = SHED_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if log_count % 20 == 0 {
                        warn!(
                            "⚠️ {}: Device '{}' never drained below {} samples, shed {} (occurrence #{})",
                            "BLOCK_ACCUMULATOR".on_yellow().green(),
                            device_id,
                            backlog.window_floor,
                            shed,
                            log_count
                        );
                    }
                }

                backlog.window_floor = usize::MAX;
                backlog.blocks_this_window = 0;
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
    const WINDOW: usize = BLOCK * 8;
    const CEILING: usize = 8192;

    fn accumulator() -> BlockAccumulator {
        BlockAccumulator::new(BLOCK, CUSHION, WINDOW, CEILING)
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
    fn a_floor_that_never_drains_is_shed() {
        let mut accumulator = accumulator();

        // A descheduled mixer drains a whole queue at once when it resumes, and
        // the mixer can never consume faster than realtime to work it back off
        accumulator.push("mic", &vec![0.5; BLOCK * 30]);

        for _ in 0..WINDOW / BLOCK {
            accumulator.push("mic", &vec![0.5; BLOCK]);
            accumulator.take_block();
        }

        // Down to the cushion, not the twenty-nine blocks it would otherwise carry
        // as delay on everything behind them for the whole session
        assert_eq!(accumulator.backlog_samples("mic"), CUSHION);
    }

    #[test]
    fn a_burst_that_drains_is_left_alone() {
        let mut accumulator = accumulator();

        // A ScreenCaptureKit tap arrives in batches and then goes quiet. Arriving
        // early costs nothing, so long as it drains before the next batch.
        let delivery = 960 * 2;

        for _ in 0..4 {
            accumulator.push("music", &vec![0.5; delivery]);
            for _ in 0..(delivery / BLOCK) + 1 {
                accumulator.take_block();
            }
        }

        // Never shed, because it kept reaching empty between batches
        assert_eq!(accumulator.backlog_samples("music"), 0);
    }

    #[test]
    fn a_delivery_is_never_cut_apart_on_arrival() {
        let mut accumulator = accumulator();

        let delivery = 960 * 2;
        accumulator.push("music", &vec![0.5; delivery]);
        accumulator.take_block();

        assert_eq!(accumulator.backlog_samples("music"), delivery - BLOCK);
    }

    #[test]
    fn the_cushion_survives_a_window() {
        let mut accumulator = accumulator();

        // Running a little ahead is what stops ordinary drift becoming a silence
        // gap, so it has to outlast the window rather than being read as surplus.
        for _ in 0..WINDOW / BLOCK {
            accumulator.push("mic", &vec![0.5; BLOCK]);
            accumulator.take_block();
        }
        accumulator.push("mic", &vec![0.5; CUSHION]);

        for _ in 0..WINDOW / BLOCK {
            accumulator.push("mic", &vec![0.5; BLOCK]);
            accumulator.take_block();
        }

        assert_eq!(accumulator.backlog_samples("mic"), CUSHION);
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
