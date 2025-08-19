// Custom hook for VU meter data management with optimized real-time updates
import { useEffect, useRef, useCallback } from 'react';
import { useMixerStore } from '../stores';
import { audioService } from '../services';
import { useThrottle } from '../utils/performance-helpers';
import { AUDIO } from '../utils/constants';

export const useVUMeterData = (isEnabled: boolean = true) => {
  const intervalRef = useRef<ReturnType<typeof setTimeout>>();
  const {
    config,
    metrics,
    masterLevels,
    batchUpdate,
    updateChannelLevels,
    updateMasterLevels,
    updateMetrics
  } = useMixerStore();

  // Throttled batch update to prevent excessive re-renders
  const throttledBatchUpdate = useThrottle((updates: {
    channelLevels?: Record<number, [number, number]>;
    masterLevels?: any;
    metrics?: any;
  }) => {
    batchUpdate(updates);
  }, AUDIO.VU_UPDATE_RATE);

  // Poll VU meter data
  const pollVUData = useCallback(async () => {
    if (!isEnabled) return;

    try {
      // Parallel API calls for better performance
      const [channelLevels, masterLevelsData, metricsData] = await Promise.all([
        audioService.getChannelLevels().catch(() => ({})),
        audioService.getMasterLevels().catch(() => [0, 0, 0, 0] as [number, number, number, number]),
        audioService.getMixerMetrics().catch(() => null)
      ]);

      // Transform master levels data
      const transformedMasterLevels = {
        left: { 
          peak_level: masterLevelsData[0] || 0, 
          rms_level: masterLevelsData[1] || 0 
        },
        right: { 
          peak_level: masterLevelsData[2] || 0, 
          rms_level: masterLevelsData[3] || 0 
        }
      };

      // Batch all updates together
      throttledBatchUpdate({
        channelLevels,
        masterLevels: transformedMasterLevels,
        metrics: metricsData
      });

      // Debug logging (only when levels are non-zero to reduce noise)
      const hasAnyLevels = Object.values(channelLevels).some(([peak, rms]) => peak > 0 || rms > 0) ||
                          masterLevelsData.some(level => level > 0);
      
      if (hasAnyLevels) {
        console.log('📊 VU Data:', {
          channels: Object.keys(channelLevels).length,
          master: `L:${masterLevelsData[0]?.toFixed(3)}/R:${masterLevelsData[2]?.toFixed(3)}`,
          cpu: metricsData?.cpu_usage?.toFixed(1)
        });
      }

    } catch (error) {
      console.error('Failed to poll VU meter data:', error);
    }
  }, [isEnabled, throttledBatchUpdate]);

  // Start/stop polling based on mixer state
  useEffect(() => {
    if (isEnabled && config) {
      console.log('🔄 Starting VU meter polling...');
      
      // Start immediate poll
      pollVUData();
      
      // Set up interval
      intervalRef.current = setInterval(pollVUData, AUDIO.VU_UPDATE_RATE);
      
      return () => {
        if (intervalRef.current) {
          console.log('⏹️ Stopping VU meter polling...');
          clearInterval(intervalRef.current);
          intervalRef.current = undefined;
        }
      };
    } else {
      // Clean up if disabled
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = undefined;
      }
    }
  }, [isEnabled, config, pollVUData]);

  // Clean up on unmount
  useEffect(() => {
    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
    };
  }, []);

  // Get channel levels by ID
  const getChannelLevels = useCallback((channelId: number) => {
    const channel = config?.channels.find(c => c.id === channelId);
    return {
      peak: channel?.peak_level || 0,
      rms: channel?.rms_level || 0
    };
  }, [config?.channels]);

  // Get all channel levels
  const getAllChannelLevels = useCallback(() => {
    if (!config) return {};
    
    return config.channels.reduce((acc, channel) => ({
      ...acc,
      [channel.id]: {
        peak: channel.peak_level,
        rms: channel.rms_level
      }
    }), {});
  }, [config]);

  return {
    // State
    metrics,
    masterLevels,
    
    // Channel level helpers
    getChannelLevels,
    getAllChannelLevels,
    
    // Manual update functions (for testing)
    updateChannelLevels,
    updateMasterLevels,
    updateMetrics,
    
    // Control polling
    pollVUData
  };
};