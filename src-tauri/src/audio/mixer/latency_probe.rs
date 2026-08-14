// End-to-end latency accounting for the capture-to-playback path
//
// Every stage that holds audio publishes how much it is currently holding,
// expressed as microseconds of audio at whatever rate and channel count that
// stage runs at. Summing a chain gives the delay a sample actually experiences
// travelling through it.
//
// Stages publish their own occupancy rather than being sampled from outside, so
// reading a snapshot never locks a queue or touches the audio path. Publishing
// is a single relaxed atomic store from a place that already computed the value.

use colored::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::info;

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

/// Handle a stage uses to publish how much audio it is holding
#[derive(Debug, Clone)]
pub struct StageGauge {
    micros: Arc<AtomicU64>,
}

impl StageGauge {
    fn new() -> Self {
        Self {
            micros: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Publish a frame count held at `sample_rate`
    pub fn set_frames(&self, frames: usize, sample_rate: u32) {
        let micros = if sample_rate == 0 {
            0
        } else {
            (frames as u64 * 1_000_000) / sample_rate as u64
        };
        self.micros.store(micros, Ordering::Relaxed);
    }

    /// Publish an interleaved sample count held at `sample_rate`
    pub fn set_samples(&self, samples: usize, channels: u16, sample_rate: u32) {
        let channels = channels.max(1) as usize;
        self.set_frames(samples / channels, sample_rate);
    }

    pub fn micros(&self) -> u64 {
        self.micros.load(Ordering::Relaxed)
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

/// What one stage of one device currently contributes
#[derive(Debug, Clone, Serialize)]
pub struct StageLatency {
    pub stage: LatencyStage,
    pub micros: u64,
}

/// Every stage one device contributes, and their sum
#[derive(Debug, Clone, Serialize)]
pub struct ChainLatency {
    pub device_id: String,
    pub stages: Vec<StageLatency>,
    pub total_micros: u64,
}

impl ChainLatency {
    /// Compact `stage=1.2ms` rendering for log lines
    pub fn describe(&self) -> String {
        self.stages
            .iter()
            .map(|stage| {
                format!(
                    "{} {:.1}ms",
                    stage.stage.label(),
                    stage.micros as f32 / 1000.0
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The full latency picture at one instant
#[derive(Debug, Clone, Serialize)]
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
    mix: StageGaugeCell,
}

/// Newtype so `LatencyProbe` can derive Default with a live gauge
#[derive(Debug)]
struct StageGaugeCell(StageGauge);

impl Default for StageGaugeCell {
    fn default() -> Self {
        Self(StageGauge::new())
    }
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
        let key = (device_id.to_string(), stage);

        if let Ok(gauges) = self.gauges.read() {
            if let Some(gauge) = gauges.get(&key) {
                return gauge.clone();
            }
        }

        match self.gauges.write() {
            Ok(mut gauges) => gauges.entry(key).or_insert_with(StageGauge::new).clone(),
            // A poisoned registry only costs this caller its readings, which is
            // never worth taking the audio pipeline down for.
            Err(poisoned) => poisoned
                .into_inner()
                .entry(key)
                .or_insert_with(StageGauge::new)
                .clone(),
        }
    }

    /// Gauge for the mix block, which every path shares
    pub fn mix_gauge(&self) -> StageGauge {
        self.mix.0.clone()
    }

    /// Drop every gauge belonging to a device that has gone away
    ///
    /// Stale entries would otherwise keep contributing their last reading to
    /// every snapshot.
    pub fn remove_device(&self, device_id: &str) {
        let mut gauges = match self.gauges.write() {
            Ok(gauges) => gauges,
            Err(poisoned) => poisoned.into_inner(),
        };
        gauges.retain(|(id, _), _| id != device_id);
    }

    pub fn snapshot(&self) -> LatencySnapshot {
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

            let micros = gauge.micros();
            entry.stages.push(StageLatency {
                stage: *stage,
                micros,
            });
            entry.total_micros += micros;
        }

        let inputs: Vec<ChainLatency> = inputs.into_values().collect();
        let outputs: Vec<ChainLatency> = outputs.into_values().collect();
        let mix_micros = self.mix.0.micros();

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
}

/// Log the latency breakdown on an interval
///
/// Returns the task handle so reporting stops when the pipeline does.
pub fn spawn_reporter(probe: Arc<LatencyProbe>, interval: Duration) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;

            let snapshot = probe.snapshot();
            if snapshot.inputs.is_empty() && snapshot.outputs.is_empty() {
                continue;
            }

            info!(
                "📏 {}: monitor path {:.1}ms (mix block {:.1}ms)",
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

    #[test]
    fn sums_each_chain_and_the_fastest_path() {
        let probe = LatencyProbe::new();

        probe
            .gauge("mic", LatencyStage::InputHardware)
            .set_frames(512, 48_000);
        probe
            .gauge("mic", LatencyStage::InputCaptureQueue)
            .set_frames(48, 48_000);
        probe
            .gauge("music", LatencyStage::InputHardware)
            .set_frames(960, 48_000);
        probe
            .gauge("headphones", LatencyStage::OutputHardware)
            .set_frames(512, 48_000);
        probe.mix_gauge().set_frames(512, 48_000);

        let snapshot = probe.snapshot();

        assert_eq!(snapshot.inputs.len(), 2);
        assert_eq!(snapshot.outputs.len(), 1);

        let mic = snapshot
            .inputs
            .iter()
            .find(|chain| chain.device_id == "mic")
            .expect("mic chain");
        assert_eq!(mic.total_micros, 10_666 + 1_000);

        // The 20ms app tap must not stand in for the monitored path.
        assert_eq!(snapshot.monitor_micros, 11_666 + 10_666 + 10_666);
    }

    #[test]
    fn a_removed_device_stops_contributing() {
        let probe = LatencyProbe::new();

        probe
            .gauge("mic", LatencyStage::InputHardware)
            .set_frames(512, 48_000);
        probe
            .gauge("headphones", LatencyStage::OutputHardware)
            .set_frames(512, 48_000);

        probe.remove_device("mic");
        let snapshot = probe.snapshot();

        assert!(snapshot.inputs.is_empty());
        assert_eq!(snapshot.monitor_micros, 0);
    }

    #[test]
    fn interleaved_samples_convert_by_channel_count() {
        let probe = LatencyProbe::new();
        let gauge = probe.gauge("mic", LatencyStage::InputCaptureQueue);

        gauge.set_samples(1024, 2, 48_000);
        assert_eq!(gauge.micros(), 10_666);

        gauge.set_samples(1024, 1, 48_000);
        assert_eq!(gauge.micros(), 21_333);
    }
}
