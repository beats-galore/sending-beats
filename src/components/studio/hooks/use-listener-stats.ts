import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';

type ListenerStats = {
  current: number | null;
  peak: number | null;
};

const EMPTY: ListenerStats = { current: null, peak: null };

/**
 * Current and peak listener counts, read from the Icecast admin endpoint.
 *
 * Only polled while streaming — the command errors when no stream manager is
 * connected, and a null count renders as "—" rather than a misleading zero.
 */
export const useListenerStats = (isLive: boolean, pollingInterval = 15000) => {
  const [stats, setStats] = useState<ListenerStats>(EMPTY);

  const fetchStats = useCallback(async () => {
    try {
      const [current, peak] = await invoke<[number, number]>('get_listener_stats');
      setStats({ current, peak });
    } catch {
      // Not connected, or the server exposes no admin stats.
      setStats(EMPTY);
    }
  }, []);

  useEffect(() => {
    if (!isLive) {
      setStats(EMPTY);
      return;
    }

    void fetchStats();
    const interval = setInterval(() => void fetchStats(), pollingInterval);
    return () => clearInterval(interval);
  }, [isLive, fetchStats, pollingInterval]);

  return stats;
};
