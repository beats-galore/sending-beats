// High-performance VU meter data streaming using Tauri channels
// Replaces the slow event system with channels designed for real-time data
import { useEffect } from 'react';
import { Channel } from '@tauri-apps/api/core';
import { invoke } from '@tauri-apps/api/core';

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

// Using the serde tag format from the Rust enum
type VUChannelData =
  | { type: 'Channel'; data: VULevelEvent }
  | { type: 'Master'; data: MasterVULevelEvent };

export const useVUChannelStream = (isEnabled = true) => {
  const batchUpdate = useMixerStore((state) => state.batchUpdate);

  useEffect(() => {
    if (!isEnabled) {
      console.log('🎧 VU channel stream disabled, skipping setup');
      return;
    }

    let channel: Channel<VUChannelData> | null = null;

    const setupChannelStream = async () => {
      try {
        console.log('🚀 Setting up VU channel stream...');

        // Create a new channel for high-performance VU data streaming
        channel = new Channel<VUChannelData>();

        // Set up message handler for incoming VU data
        channel.onmessage = (data: VUChannelData) => {
          if (data.type === 'Channel') {
            // Handle channel VU data
            const vuData = data.data;

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
          } else if (data.type === 'Master') {
            // Handle master VU data
            const vuData = data.data;

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
          }
        };

        // Initialize the high-performance VU channel streaming
        await invoke('initialize_vu_channels', { channel });

        console.log('✅ VU channel streaming initialized successfully');
      } catch (error) {
        console.error('❌ Failed to initialize VU channel streaming:', error);
        // Fall back to event system if channel initialization fails
        console.log('📡 VU channels unavailable, event system will be used instead');
      }
    };

    setupChannelStream();

    // Cleanup function
    return () => {
      if (channel) {
        // Note: Tauri channels are automatically cleaned up when the component unmounts
        // The Rust side will detect when the channel is closed
        console.log('🧹 VU channel streaming cleaned up');
      }
    };
  }, [isEnabled, batchUpdate]);

  // This hook manages high-performance channel streaming, doesn't return data
  // Components should use focused hooks to read specific data they need
};