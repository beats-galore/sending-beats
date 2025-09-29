// Event-driven VU level updates using Tauri events
import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';

import { useMixerStore } from '../stores';

type VULevelEvent = {
  device_id: string;
  channel: number;
  peak_left: number;
  peak_right: number;
  rms_left: number;
  rms_right: number;
  is_stereo: boolean;
  timestamp: number;
};

type MasterVULevelEvent = {
  peak_left: number;
  peak_right: number;
  rms_left: number;
  rms_right: number;
  timestamp: number;
};

export const useVULevelEvents = (isEnabled = true) => {
  const batchUpdate = useMixerStore((state) => state.batchUpdate);

  useEffect(() => {
    if (!isEnabled) {
      return;
    }

    let channelUnlisten: (() => void) | null = null;
    let masterUnlisten: (() => void) | null = null;

    const setupListeners = async () => {
      // Listen for channel VU level events
      channelUnlisten = await listen<VULevelEvent>('vu-channel-level', (event) => {
        const vuData = event.payload;

        // Debug: Check if events are being received with delay
        if (Math.random() < 0.001) { // Log 0.1% of events
          console.log('🎧 Frontend received VU event at', new Date().toLocaleTimeString(), vuData.peak_left.toFixed(1));
        }

        // Convert dB values to 0-1 linear range for UI components
        const dbToLinear = (db: number) => Math.pow(10, db / 20);

        // Convert to the format expected by the mixer store
        const channelLevels: Record<number, [number, number, number, number]> = {
          [vuData.channel]: [
            dbToLinear(vuData.peak_left),
            dbToLinear(vuData.rms_left),
            dbToLinear(vuData.peak_right),
            dbToLinear(vuData.rms_right),
          ],
        };

        batchUpdate({ channelLevels });
      });

      // Listen for master VU level events
      masterUnlisten = await listen<MasterVULevelEvent>('vu-master-level', (event) => {
        const vuData = event.payload;

        // Convert dB values to 0-1 linear range for UI components
        const dbToLinear = (db: number) => Math.pow(10, db / 20);

        // Convert to the format expected by the mixer store
        const masterLevels = {
          left: {
            peak_level: dbToLinear(vuData.peak_left),
            rms_level: dbToLinear(vuData.rms_left),
          },
          right: {
            peak_level: dbToLinear(vuData.peak_right),
            rms_level: dbToLinear(vuData.rms_right),
          },
        };

        batchUpdate({ masterLevels });
      });

    };

    setupListeners().catch(console.error);

    // Cleanup function
    return () => {
      if (channelUnlisten) {
        channelUnlisten();
      }

      if (masterUnlisten) {
        masterUnlisten();
      }
    };
  }, [isEnabled, batchUpdate]);

  // This hook manages event listeners, doesn't return data
  // Components should use focused hooks to read specific data they need
};