// Per-stage latency reported by the audio pipeline's probe
//
// Mirrors `LatencyStage` / `LatencySnapshot` in
// src-tauri/src/audio/mixer/latency_probe.rs.

export const LatencyStage = [
  'input_hardware',
  'input_capture_queue',
  'input_accumulator',
  'input_resampler',
  'input_mix_queue',
  'input_backlog',
  'output_mix_queue',
  'output_accumulator',
  'output_resampler',
  'output_hardware_queue',
  'output_hardware',
] as const;
export type LatencyStage = (typeof LatencyStage)[number];

export type StageLatency = {
  stage: LatencyStage;
  micros: number;
};

export type ChainLatency = {
  device_id: string;
  stages: StageLatency[];
  total_micros: number;
};

export type LatencySnapshot = {
  inputs: ChainLatency[];
  outputs: ChainLatency[];
  /** The mix block itself, shared by every path */
  mix_micros: number;
  /**
   * Fastest currently available capture-to-playback path — the one a monitored
   * microphone takes. Zero until both an input and an output are running.
   */
  monitor_micros: number;
};

/** Human-readable stage names, in the order audio passes through them */
export const LATENCY_STAGE_LABELS: Record<LatencyStage, string> = {
  input_hardware: 'Input hardware',
  input_capture_queue: 'Capture queue',
  input_accumulator: 'Input accumulator',
  input_resampler: 'Input resampler',
  input_mix_queue: 'Mix queue',
  input_backlog: 'Mixer backlog',
  output_mix_queue: 'Output queue',
  output_accumulator: 'Output accumulator',
  output_resampler: 'Output resampler',
  output_hardware_queue: 'Hardware queue',
  output_hardware: 'Output hardware',
};
