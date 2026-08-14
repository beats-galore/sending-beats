// End-to-end latency accounting for the capture-to-playback path
//
// Every stage that holds audio publishes how much it is holding. What a stage
// contributes is not that reading but its *mean* over time: by Little's Law the
// mean time a sample spends in a stage is its mean occupancy divided by the rate
// audio flows through it, so time-weighted means are the quantity that can be
// summed along a path to give end-to-end latency.
//
// The distinction matters. A queue that a burst fills and a worker immediately
// drains is empty for most of its cycle; sampling it at the instant it is full
// reports the burst size, which is the buffer upstream of it and already counted
// there. Weighting by how long each level was actually held reports the ~1ms the
// audio really waited.
//
// Stages publish their own occupancy rather than being sampled from outside, so
// nothing here locks a queue or touches the audio path. Each gauge has exactly
// one writer and is drained by exactly one sampler.

use colored::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};
use tracing::info;

/// Common zero point, so every gauge can keep its timestamps in a plain atomic
fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

fn now_nanos() -> u64 {
    epoch().elapsed().as_nanos() as u64
}

/// A stage of the pipeline that holds audio, in path order
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LatencyStage {
    /// Capture device's own delay: hardware buffer, presentation latency, safety offset
    InputHardware,
    /// CoreAudio capture callback → InputWorker
    InputCaptureQueue,
    /// InputWorker's pre-resample accumulation
    InputAccumulator,
    /// Group delay of the input worker's resampler
    InputResampler,
    /// InputWorker → MixingLayer
    InputMixQueue,
    /// Per-device backlog waiting in the mixer's block accumulator
    InputBacklog,
    /// MixingLayer → OutputWorker
    OutputMixQueue,
    /// OutputWorker's pre-resample accumulation
    OutputAccumulator,
    /// Group delay of the output worker's resampler
    OutputResampler,
    /// OutputWorker → CoreAudio render callback
    OutputHardwareQueue,
    /// Playback device's own delay: hardware buffer, presentation latency, safety offset
    OutputHardware,
}

/// Which half of the path a stage belongs to, split at the mix point
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyChain {
    Input,
    Output,
}

impl LatencyStage {
    pub fn chain(self) -> LatencyChain {
        match self {
            Self::InputHardware
            | Self::InputCaptureQueue
            | Self::InputAccumulator
            | Self::InputResampler
            | Self::InputMixQueue
            | Self::InputBacklog => LatencyChain::Input,
            Self::OutputMixQueue
            | Self::OutputAccumulator
            | Self::OutputResampler
            | Self::OutputHardwareQueue
            | Self::OutputHardware => LatencyChain::Output,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::InputHardware => "in_hw",
            Self::InputCaptureQueue => "capture_q",
            Self::InputAccumulator => "in_accum",
            Self::InputResampler => "in_resamp",
            Self::InputMixQueue => "mix_q",
            Self::InputBacklog => "backlog",
            Self::OutputMixQueue => "out_q",
            Self::OutputAccumulator => "out_accum",
            Self::OutputResampler => "out_resamp",
            Self::OutputHardwareQueue => "hw_q",
            Self::OutputHardware => "out_hw",
        }
    }
}

/// What a gauge held over one sampling window
#[derive(Debug, Clone, Copy, Default)]
pub struct StageStats {
    /// Time-weighted mean, the figure that sums along a path
    pub mean_micros: u64,
    /// Highest single reading, which shows burst size and stalls
    pub peak_micros: u64,
}

#[derive(Debug, Default)]
struct GaugeState {
    /// Occupancy as of the last publish, in microseconds of audio
    current: AtomicU64,
    /// When that value was published, nanoseconds since the epoch
    since: AtomicU64,
    /// Occupancy integrated over time: microseconds of audio × nanoseconds
    integral: AtomicU64,
    /// Highest value published in the window
    peak: AtomicU64,
    /// When the window opened
    window_start: AtomicU64,
}

/// Handle a stage uses to publish how much audio it is holding
#[derive(Debug, Clone)]
pub struct StageGauge {
    state: Arc<GaugeState>,
}

impl StageGauge {
    fn new(now: u64) -> Self {
        let state = GaugeState::default();
        state.since.store(now, Ordering::Relaxed);
        state.window_start.store(now, Ordering::Relaxed);

        Self {
            state: Arc::new(state),
        }
    }

    /// Publish a frame count held at `sample_rate`
    pub fn set_frames(&self, frames: usize, sample_rate: u32) {
        let micros = if sample_rate == 0 {
            0
        } else {
            (frames as u64 * 1_000_000) / sample_rate as u64
        };
        self.set_micros_at(micros, now_nanos());
    }

    /// Publish an interleaved sample count held at `sample_rate`
    pub fn set_samples(&self, samples: usize, channels: u16, sample_rate: u32) {
        let channels = channels.max(1) as usize;
        self.set_frames(samples / channels, sample_rate);
    }

    /// Publish at an explicit time, so tests can drive the clock
    ///
    /// The value replaces what was there, and however long the *previous* value
    /// was held is what gets integrated. That is what makes a level held for a
    /// long time count for more than one held briefly.
    fn set_micros_at(&self, micros: u64, now: u64) {
        let held_since = self.state.since.swap(now, Ordering::Relaxed);
        let previous = self.state.current.swap(micros, Ordering::Relaxed);
        let held_for = now.saturating_sub(held_since);

        self.state
            .integral
            .fetch_add(previous.saturating_mul(held_for), Ordering::Relaxed);
        self.state.peak.fetch_max(micros, Ordering::Relaxed);
    }

    /// Close the window and start a new one
    fn drain_at(&self, now: u64) -> StageStats {
        let held_since = self.state.since.swap(now, Ordering::Relaxed);
        let current = self.state.current.load(Ordering::Relaxed);
        let held_for = now.saturating_sub(held_since);

        let integral = self
            .state
            .integral
            .swap(0, Ordering::Relaxed)
            .saturating_add(current.saturating_mul(held_for));

        let window_start = self.state.window_start.swap(now, Ordering::Relaxed);
        let elapsed = now.saturating_sub(window_start);

        // With no window to average over, the last reading is all there is
        let mean_micros = if elapsed == 0 {
            current
        } else {
            integral / elapsed
        };

        StageStats {
            mean_micros,
            peak_micros: self.state.peak.swap(0, Ordering::Relaxed).max(current),
        }
    }
}

/// The gauges a pipeline worker publishes, in the order audio passes through it
///
/// Every RTRB queue on the path is published by exactly one worker: the one that
/// holds its consumer publishes it as `inbound`, and that same worker publishes
/// the queue it produces into as `outbound`. Nothing is counted twice.
#[derive(Debug, Clone)]
pub struct WorkerLatencyGauges {
    pub inbound: StageGauge,
    pub accumulator: StageGauge,
    pub resampler: StageGauge,
    pub outbound: StageGauge,
}

impl WorkerLatencyGauges {
    pub fn for_input(probe: &LatencyProbe, device_id: &str) -> Self {
        Self {
            inbound: probe.gauge(device_id, LatencyStage::InputCaptureQueue),
            accumulator: probe.gauge(device_id, LatencyStage::InputAccumulator),
            resampler: probe.gauge(device_id, LatencyStage::InputResampler),
            outbound: probe.gauge(device_id, LatencyStage::InputMixQueue),
        }
    }

    pub fn for_output(probe: &LatencyProbe, device_id: &str) -> Self {
        Self {
            inbound: probe.gauge(device_id, LatencyStage::OutputMixQueue),
            accumulator: probe.gauge(device_id, LatencyStage::OutputAccumulator),
            resampler: probe.gauge(device_id, LatencyStage::OutputResampler),
            outbound: probe.gauge(device_id, LatencyStage::OutputHardwareQueue),
        }
    }
}

/// What one stage of one device contributed over the last window
#[derive(Debug, Clone, Serialize)]
pub struct StageLatency {
    pub stage: LatencyStage,
    pub mean_micros: u64,
    pub peak_micros: u64,
}

/// Every stage one device contributes, and the sum of their means
#[derive(Debug, Clone, Serialize)]
pub struct ChainLatency {
    pub device_id: String,
    pub stages: Vec<StageLatency>,
    pub total_micros: u64,
}

impl ChainLatency {
    /// Compact `stage mean/peak` rendering for log lines
    pub fn describe(&self) -> String {
        self.stages
            .iter()
            .map(|stage| {
                format!(
                    "{} {:.1}/{:.1}ms",
                    stage.stage.label(),
                    stage.mean_micros as f32 / 1000.0,
                    stage.peak_micros as f32 / 1000.0
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The latency picture over one sampling window
#[derive(Debug, Clone, Default, Serialize)]
pub struct LatencySnapshot {
    pub inputs: Vec<ChainLatency>,
    pub outputs: Vec<ChainLatency>,
    /// The mix block itself, shared by every path
    pub mix_micros: u64,
    /// Fastest currently available capture-to-playback path
    ///
    /// Sources differ in how coarsely they deliver — a ScreenCaptureKit tap
    /// cannot beat its own 20ms cadence — so the slowest input says nothing about
    /// whether live monitoring works. This is the path a monitored microphone
    /// takes, and the number the target of 10-15ms applies to.
    pub monitor_micros: u64,
}

impl LatencySnapshot {
    pub fn monitor_ms(&self) -> f32 {
        self.monitor_micros as f32 / 1000.0
    }
}

/// Registry of every stage gauge in the pipeline
#[derive(Debug, Default)]
pub struct LatencyProbe {
    gauges: RwLock<BTreeMap<(String, LatencyStage), StageGauge>>,
    mix: OnceLock<StageGauge>,
    /// Last completed window, so every reader sees the same averaged figures
    /// rather than racing each other to drain the gauges
    latest: RwLock<LatencySnapshot>,
}

impl LatencyProbe {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Gauge for one stage of one device, created on first request
    ///
    /// Callers hold onto the returned handle; this is not meant to be called
    /// from a processing loop.
    pub fn gauge(&self, device_id: &str, stage: LatencyStage) -> StageGauge {
        self.gauge_at(device_id, stage, now_nanos())
    }

    /// Create at an explicit time, so tests can drive the clock
    fn gauge_at(&self, device_id: &str, stage: LatencyStage, now: u64) -> StageGauge {
        let key = (device_id.to_string(), stage);

        if let Ok(gauges) = self.gauges.read() {
            if let Some(gauge) = gauges.get(&key) {
                return gauge.clone();
            }
        }

        match self.gauges.write() {
            Ok(mut gauges) => gauges
                .entry(key)
                .or_insert_with(|| StageGauge::new(now))
                .clone(),
            // A poisoned registry only costs this caller its readings, which is
            // never worth taking the audio pipeline down for.
            Err(poisoned) => poisoned
                .into_inner()
                .entry(key)
                .or_insert_with(|| StageGauge::new(now))
                .clone(),
        }
    }

    /// Gauge for the mix block, which every path shares
    pub fn mix_gauge(&self) -> StageGauge {
        self.mix_gauge_at(now_nanos())
    }

    fn mix_gauge_at(&self, now: u64) -> StageGauge {
        self.mix.get_or_init(|| StageGauge::new(now)).clone()
    }

    /// Drop every gauge belonging to a device that has gone away
    ///
    /// Stale entries would otherwise keep contributing their last reading to
    /// every window.
    pub fn remove_device(&self, device_id: &str) {
        let mut gauges = match self.gauges.write() {
            Ok(gauges) => gauges,
            Err(poisoned) => poisoned.into_inner(),
        };
        gauges.retain(|(id, _), _| id != device_id);
    }

    /// Close the current window and cache what it measured
    ///
    /// Only the sampler calls this. Draining resets each gauge, so two callers
    /// would each see a fraction of the window and neither would see the whole.
    pub fn sample(&self) -> LatencySnapshot {
        self.sample_at(now_nanos())
    }

    fn sample_at(&self, now: u64) -> LatencySnapshot {
        let snapshot = self.measure_at(now);

        match self.latest.write() {
            Ok(mut latest) => *latest = snapshot.clone(),
            Err(poisoned) => *poisoned.into_inner() = snapshot.clone(),
        }

        snapshot
    }

    fn measure_at(&self, now: u64) -> LatencySnapshot {
        let gauges = match self.gauges.read() {
            Ok(gauges) => gauges,
            Err(poisoned) => poisoned.into_inner(),
        };

        let mut inputs: BTreeMap<String, ChainLatency> = BTreeMap::new();
        let mut outputs: BTreeMap<String, ChainLatency> = BTreeMap::new();

        // Keys are ordered by device then stage, so each chain's stages come out
        // in path order without sorting.
        for ((device_id, stage), gauge) in gauges.iter() {
            let chain = match stage.chain() {
                LatencyChain::Input => &mut inputs,
                LatencyChain::Output => &mut outputs,
            };

            let entry = chain
                .entry(device_id.clone())
                .or_insert_with(|| ChainLatency {
                    device_id: device_id.clone(),
                    stages: Vec::new(),
                    total_micros: 0,
                });

            let stats = gauge.drain_at(now);
            entry.stages.push(StageLatency {
                stage: *stage,
                mean_micros: stats.mean_micros,
                peak_micros: stats.peak_micros,
            });
            entry.total_micros += stats.mean_micros;
        }

        let inputs: Vec<ChainLatency> = inputs.into_values().collect();
        let outputs: Vec<ChainLatency> = outputs.into_values().collect();
        let mix_micros = self
            .mix
            .get()
            .map_or(0, |gauge| gauge.drain_at(now).mean_micros);

        let fastest_input = inputs.iter().map(|chain| chain.total_micros).min();
        let fastest_output = outputs.iter().map(|chain| chain.total_micros).min();

        // Only meaningful once both halves of a path exist.
        let monitor_micros = match (fastest_input, fastest_output) {
            (Some(input), Some(output)) => input + mix_micros + output,
            _ => 0,
        };

        LatencySnapshot {
            inputs,
            outputs,
            mix_micros,
            monitor_micros,
        }
    }

    /// The last completed window, without disturbing the one in progress
    pub fn snapshot(&self) -> LatencySnapshot {
        match self.latest.read() {
            Ok(latest) => latest.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

/// Sample the probe on a fixed interval, logging every `log_every` windows
///
/// One task owns sampling so every reader shares the same averaged figures.
/// Returns the handle so sampling stops when the pipeline does.
pub fn spawn_reporter(
    probe: Arc<LatencyProbe>,
    interval: Duration,
    log_every: u32,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut windows = 0u32;

        loop {
            tokio::time::sleep(interval).await;

            let snapshot = probe.sample();
            windows += 1;

            if windows % log_every.max(1) != 0 {
                continue;
            }
            if snapshot.inputs.is_empty() && snapshot.outputs.is_empty() {
                continue;
            }

            info!(
                "📏 {}: monitor path {:.1}ms (mix block {:.1}ms) — mean/peak per stage",
                "LATENCY_PROBE".on_white().black(),
                snapshot.monitor_ms(),
                snapshot.mix_micros as f32 / 1000.0
            );

            for chain in snapshot.inputs.iter() {
                info!(
                    "📏 {}:   in  '{}' {:.1}ms — {}",
                    "LATENCY_PROBE".on_white().black(),
                    chain.device_id,
                    chain.total_micros as f32 / 1000.0,
                    chain.describe()
                );
            }

            for chain in snapshot.outputs.iter() {
                info!(
                    "📏 {}:   out '{}' {:.1}ms — {}",
                    "LATENCY_PROBE".on_white().black(),
                    chain.device_id,
                    chain.total_micros as f32 / 1000.0,
                    chain.describe()
                );
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000; // nanoseconds

    #[test]
    fn a_level_counts_for_as_long_as_it_was_held() {
        let gauge = StageGauge::new(0);

        // 10ms of audio held for one millisecond, then empty for nine.
        gauge.set_micros_at(10_000, 0);
        gauge.set_micros_at(0, MS);

        let stats = gauge.drain_at(10 * MS);

        // A tenth of the window at 10ms, the rest at nothing.
        assert_eq!(stats.mean_micros, 1_000);
        // The burst is still visible, it just does not stand in for the mean.
        assert_eq!(stats.peak_micros, 10_000);
    }

    #[test]
    fn a_queue_that_never_drains_reports_what_it_holds() {
        let gauge = StageGauge::new(0);

        // Oscillating near full, the way a back-pressured output ring does.
        gauge.set_micros_at(42_000, 0);
        gauge.set_micros_at(32_000, 5 * MS);

        let stats = gauge.drain_at(10 * MS);

        assert_eq!(stats.mean_micros, 37_000);
        assert_eq!(stats.peak_micros, 42_000);
    }

    #[test]
    fn a_window_closes_when_it_is_drained() {
        let gauge = StageGauge::new(0);

        gauge.set_micros_at(10_000, 0);
        assert_eq!(gauge.drain_at(MS).mean_micros, 10_000);

        // The next window starts empty rather than inheriting the last one.
        gauge.set_micros_at(0, MS);
        assert_eq!(gauge.drain_at(2 * MS).mean_micros, 0);
        assert_eq!(gauge.drain_at(3 * MS).peak_micros, 0);
    }

    #[test]
    fn sums_each_chain_and_the_fastest_path() {
        let probe = LatencyProbe::new();

        probe
            .gauge_at("mic", LatencyStage::InputHardware, 0)
            .set_micros_at(10_000, 0);
        probe
            .gauge_at("mic", LatencyStage::InputCaptureQueue, 0)
            .set_micros_at(1_000, 0);
        probe
            .gauge_at("music", LatencyStage::InputHardware, 0)
            .set_micros_at(20_000, 0);
        probe
            .gauge_at("headphones", LatencyStage::OutputHardware, 0)
            .set_micros_at(10_000, 0);
        probe.mix_gauge_at(0).set_micros_at(10_000, 0);

        let snapshot = probe.sample_at(MS);

        assert_eq!(snapshot.inputs.len(), 2);
        assert_eq!(snapshot.outputs.len(), 1);

        let mic = snapshot
            .inputs
            .iter()
            .find(|chain| chain.device_id == "mic")
            .expect("mic chain");
        assert_eq!(mic.total_micros, 11_000);

        // The 20ms app tap must not stand in for the monitored path.
        assert_eq!(snapshot.monitor_micros, 11_000 + 10_000 + 10_000);
    }

    #[test]
    fn readers_share_the_last_completed_window() {
        let probe = LatencyProbe::new();

        probe
            .gauge_at("mic", LatencyStage::InputHardware, 0)
            .set_micros_at(10_000, 0);
        probe
            .gauge_at("headphones", LatencyStage::OutputHardware, 0)
            .set_micros_at(10_000, 0);

        // Nothing has been sampled yet, so there is nothing to report.
        assert_eq!(probe.snapshot().monitor_micros, 0);

        probe.sample_at(MS);

        // Two readers see the same figures rather than draining it from each other.
        assert_eq!(probe.snapshot().monitor_micros, 20_000);
        assert_eq!(probe.snapshot().monitor_micros, 20_000);
    }

    #[test]
    fn a_removed_device_stops_contributing() {
        let probe = LatencyProbe::new();

        probe
            .gauge_at("mic", LatencyStage::InputHardware, 0)
            .set_micros_at(10_000, 0);
        probe
            .gauge_at("headphones", LatencyStage::OutputHardware, 0)
            .set_micros_at(10_000, 0);

        probe.remove_device("mic");
        let snapshot = probe.sample_at(MS);

        assert!(snapshot.inputs.is_empty());
        assert_eq!(snapshot.monitor_micros, 0);
    }

    #[test]
    fn interleaved_samples_convert_by_channel_count() {
        let probe = LatencyProbe::new();
        let gauge = probe.gauge("mic", LatencyStage::InputCaptureQueue);

        gauge.set_samples(1024, 2, 48_000);
        assert_eq!(gauge.state.current.load(Ordering::Relaxed), 10_666);

        gauge.set_samples(1024, 1, 48_000);
        assert_eq!(gauge.state.current.load(Ordering::Relaxed), 21_333);
    }
}
