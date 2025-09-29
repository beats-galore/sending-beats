// High-performance VU meter data streaming - testing channel approach
import { useVUChannelStream } from './use-vu-channel-stream';
import { useMixerStore } from '../stores';

export const useVUMeterData = (isEnabled = true) => {
  // Check if mixer is configured to enable streaming
  const hasConfig = useMixerStore((state) => state.config !== null);

  // Test the new channel-based VU streaming for improved responsiveness
  useVUChannelStream(isEnabled && hasConfig);

  // This hook manages high-performance channel streaming
  // Components should use focused hooks to read specific data they need
};
