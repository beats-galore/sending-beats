// Custom hook for VU meter data management with optimized real-time updates
import { useEffect, useRef, useCallback } from 'react';

import { audioService } from '../services';
import { useMixerStore } from '../stores';
import { AUDIO } from '../utils/constants';
import { useThrottle } from '../utils/performance-helpers';

export const useVUMeterData = (isEnabled = true) => {
  const intervalRef = useRef<ReturnType<typeof setTimeout>>();
  // Only read what we need for polling logic - no data that changes from polling
  const hasConfig = useMixerStore((state) => state.config !== null);
  const batchUpdate = useMixerStore((state) => state.batchUpdate);

  // Throttled batch update to prevent excessive re-renders - memoized at 30fps
  const throttledBatchUpdate = useThrottle(
    useCallback(
      (updates: {
        channelLevels?: Record<number, [number, number, number, number]>;
        masterLevels?: any;
        metrics?: any;
      }) => {
        // Additional throttling check - only update if significant change
        batchUpdate(updates);
      },
      [batchUpdate]
    ),
    AUDIO.VU_THROTTLE_RATE // 33ms = 30fps for smooth but efficient updates
  );

  // Poll VU meter data
  const pollVUData = useCallback(async () => {
    if (!isEnabled) return;

    try {
      // Parallel API calls for better performance
      const [channelLevels, masterLevelsData, metricsData] = await Promise.all([
        audioService.getChannelLevels().catch(() => ({})),
        audioService
          .getMasterLevels()
          .catch(() => [0, 0, 0, 0] as [number, number, number, number]),
        audioService.getMixerMetrics().catch(() => null),
      ]);

      // Transform master levels data
      const transformedMasterLevels = {
        left: {
          peak_level: masterLevelsData[0] || 0,
          rms_level: masterLevelsData[1] || 0,
        },
        right: {
          peak_level: masterLevelsData[2] || 0,
          rms_level: masterLevelsData[3] || 0,
        },
      };

      // Batch all updates together
      throttledBatchUpdate({
        channelLevels,
        masterLevels: transformedMasterLevels,
        metrics: metricsData,
      });

      // Debug logging (only when levels are non-zero to reduce noise)
      const hasAnyLevels =
        Object.values(channelLevels).some(([peak, rms]) => peak > 0 || rms > 0) ||
        masterLevelsData.some((level) => level > 0);

      if (hasAnyLevels) {
        // console.debug('📊 VU Data:', {
        //   channels: Object.keys(channelLevels).length,
        //   master: `L:${masterLevelsData[0]?.toFixed(3)}/R:${masterLevelsData[2]?.toFixed(3)}`,
        //   cpu: metricsData?.cpu_usage?.toFixed(1),
        // });
      }
    } catch (error) {
      console.error('Failed to poll VU meter data:', error);
    }
  }, [isEnabled, throttledBatchUpdate]);

  // Start/stop polling based on mixer state
  useEffect(() => {
    console.debug('📊 VU meter useEffect triggered:', { isEnabled, hasConfig });

    if (isEnabled && hasConfig) {
      console.debug('🔄 Starting VU meter polling...');

      // Start immediate poll
      pollVUData();

      // Set up interval
      intervalRef.current = setInterval(pollVUData, AUDIO.VU_UPDATE_RATE);

      return () => {
        if (intervalRef.current) {
          console.debug('⏹️ Stopping VU meter polling...');
          clearInterval(intervalRef.current);
          intervalRef.current = undefined;
        }
      };
    }
    console.debug('🚫 VU polling disabled or no config');
    // Clean up if disabled
    if (intervalRef.current) {
      console.debug('🛑 Cleaning up VU meter polling...');
      clearInterval(intervalRef.current);
      intervalRef.current = undefined;
    }
  }, [isEnabled, hasConfig, pollVUData]);

  // Clean up on unmount
  useEffect(() => {
    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
    };
  }, []);

  // This hook only manages polling, doesn't return data
  // Components should use focused hooks to read specific data they need
};
