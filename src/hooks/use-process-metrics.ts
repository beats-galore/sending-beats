// Polls what the app process is costing in CPU and memory
import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';

import type { ProcessMetrics } from '../types/process-metrics.types';

/**
 * Kept above sysinfo's 200ms minimum: CPU is a difference between refreshes, and
 * sampling faster narrows the window it is measured over rather than tracking
 * the process more closely.
 */
const POLL_INTERVAL_MS = 1000;

export const useProcessMetrics = () => {
  const [metrics, setMetrics] = useState<ProcessMetrics | null>(null);

  useEffect(() => {
    let cancelled = false;

    const poll = async () => {
      try {
        const next = await invoke<ProcessMetrics>('get_process_metrics');
        if (!cancelled) {
          setMetrics(next);
        }
      } catch {
        // Keeping the last reading beats flashing an em dash between polls
      }
    };

    void poll();
    const timer = setInterval(() => void poll(), POLL_INTERVAL_MS);

    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  return metrics;
};
