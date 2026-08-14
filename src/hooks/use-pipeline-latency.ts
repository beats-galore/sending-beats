// Polls the audio pipeline for what it is actually costing, stage by stage
import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';

import type { LatencySnapshot } from '../types/latency.types';
import { useMixerRunningState } from './use-mixer-running-state';


const POLL_INTERVAL_MS = 1000;

export const usePipelineLatency = () => {
  const [snapshot, setSnapshot] = useState<LatencySnapshot | null>(null);
  const isRunning = useMixerRunningState();

  useEffect(() => {
    if (!isRunning) {
      setSnapshot(null);
      return;
    }

    let cancelled = false;

    const poll = async () => {
      try {
        const next = await invoke<LatencySnapshot>('get_pipeline_latency');
        if (!cancelled) {
          setSnapshot(next);
        }
      } catch {
        // The pipeline reports nothing while it is starting or tearing down.
        // Keeping the last reading beats flashing an em dash between polls.
      }
    };

    void poll();
    const timer = setInterval(() => void poll(), POLL_INTERVAL_MS);

    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [isRunning]);

  return snapshot;
};
