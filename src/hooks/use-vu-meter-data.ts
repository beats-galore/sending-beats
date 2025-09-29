// Event-driven VU meter data management - replaced polling with real-time events
import { useVULevelEvents } from './use-vu-level-events';
import { useMixerStore } from '../stores';

export const useVUMeterData = (isEnabled = true) => {
  // Check if mixer is configured to enable event listening
  const hasConfig = useMixerStore((state) => state.config !== null);

  // Use event-driven VU levels instead of polling
  useVULevelEvents(isEnabled && hasConfig);

  // This hook only manages event listeners, doesn't return data
  // Components should use focused hooks to read specific data they need
};
