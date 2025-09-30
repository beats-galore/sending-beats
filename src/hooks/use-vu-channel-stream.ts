// High-performance VU meter data streaming using Tauri channels
// Replaces the slow event system with channels designed for real-time data
import { Channel, invoke } from '@tauri-apps/api/core';
import { useEffect } from 'react';

import { useVUMeterStore } from '../stores';

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
  const batchUpdate = useVUMeterStore((state) => state.batchUpdate);

  useEffect(() => {
    if (!isEnabled) {
      console.log('🎧 VU channel stream disabled, skipping setup');
      return;
    }

    let channel: Channel<VUChannelData> | null = null;
    let messageCount = 0;
    let lastLogTime = Date.now();
    let pendingChannelLevels: Record<number, [number, number, number, number]> = {};
    let pendingMasterLevels: {
      left: { peak_level: number; rms_level: number };
      right: { peak_level: number; rms_level: number };
    } | null = null;
    let rafId: number | null = null;

    // Batch all VU updates into a single store update per animation frame
    const flushUpdates = () => {
      const updates: {
        channelLevels?: Record<number, [number, number, number, number]>;
        masterLevels?: {
          left: { peak_level: number; rms_level: number };
          right: { peak_level: number; rms_level: number };
        };
      } = {};

      if (Object.keys(pendingChannelLevels).length > 0) {
        updates.channelLevels = pendingChannelLevels;
        pendingChannelLevels = {};
      }

      if (pendingMasterLevels) {
        updates.masterLevels = pendingMasterLevels;
        pendingMasterLevels = null;
      }

      if (Object.keys(updates).length > 0) {
        batchUpdate(updates);
      }

      rafId = null;
    };

    const setupChannelStream = async () => {
      try {
        console.log('🚀 Setting up VU channel stream...');

        // Create a new channel for high-performance VU data streaming
        channel = new Channel<VUChannelData>();

        let firstMessageTime = 0;
        let lastMessageTime = 0;

        // Set up message handler for incoming VU data
        channel.onmessage = (data: VUChannelData) => {
          const now = Date.now();
          if (messageCount === 0) {
            firstMessageTime = now;
          }
          lastMessageTime = now;
          messageCount++;

          // Log timing every 10 messages at 1fps
          if (messageCount % 10 === 0) {
            const totalTime = now - firstMessageTime;
            const timeSinceLastLog = now - lastLogTime;
            const messagesPerSecond = (10 / timeSinceLastLog) * 1000;
            const avgDelay = totalTime / messageCount;

            console.log(
              `📊 VU_CHANNEL_DEBUG: msg #${messageCount}, ${messagesPerSecond.toFixed(1)}/sec, avg delay: ${avgDelay.toFixed(0)}ms, total time: ${totalTime}ms`
            );

            lastLogTime = now;
          }

          if (data.type === 'Channel') {
            // Handle channel VU data - accumulate in pending batch
            const vuData = data.data;

            // Convert dB values to 0-1 linear range for UI components
            const dbToLinear = (db: number) => 10 ** (db / 20);

            pendingChannelLevels[vuData.channel] = [
              dbToLinear(vuData.peak_left),
              dbToLinear(vuData.rms_left),
              dbToLinear(vuData.peak_right),
              dbToLinear(vuData.rms_right),
            ];

            if (messageCount % 500 === 0) {
              console.log(`📥 Received channel ${vuData.channel} VU data:`, vuData);
            }
          } else if (data.type === 'Master') {
            // Handle master VU data - accumulate in pending batch
            const vuData = data.data;

            // Convert dB values to 0-1 linear range for UI components
            const dbToLinear = (db: number) => 10 ** (db / 20);

            pendingMasterLevels = {
              left: {
                peak_level: dbToLinear(vuData.peak_left),
                rms_level: dbToLinear(vuData.rms_left),
              },
              right: {
                peak_level: dbToLinear(vuData.peak_right),
                rms_level: dbToLinear(vuData.rms_right),
              },
            };

            if (messageCount % 500 === 0) {
              console.log('📥 Received master VU data:', vuData);
            }
          }

          // Schedule flush on next animation frame (only once per frame)
          if (rafId === null) {
            rafId = requestAnimationFrame(flushUpdates);
            if (messageCount % 1000 === 0) {
              console.log('⏰ Scheduled RAF flush, pending:', {
                channels: Object.keys(pendingChannelLevels).length,
                hasMaster: !!pendingMasterLevels,
              });
            }
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

    void setupChannelStream();

    // Cleanup function
    return () => {
      if (rafId !== null) {
        cancelAnimationFrame(rafId);
      }
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
