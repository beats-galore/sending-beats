// What the app process itself is costing
//
// Mirrors `ProcessMetrics` in src-tauri/src/process_metrics.rs.

export type ProcessMetrics = {
  /**
   * Percentage of a single core, summed across cores, so it reads above 100
   * when more than one thread is busy. Matches Activity Monitor.
   */
  cpu_percent: number;
  /** Resident set size, in bytes */
  memory_bytes: number;
  /** False until CPU has been sampled twice, since it is a difference over time */
  cpu_ready: boolean;
};

/** Bytes as the nearest sensible unit, for a readout with limited room */
export const formatBytes = (bytes: number): string => {
  if (bytes >= 1024 ** 3) {
    return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  }
  if (bytes >= 1024 ** 2) {
    return `${Math.round(bytes / 1024 ** 2)} MB`;
  }
  return `${Math.round(bytes / 1024)} KB`;
};
