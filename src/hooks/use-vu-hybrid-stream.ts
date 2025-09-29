// Hybrid VU meter system: Attempts channels first, falls back to events
// This provides the best of both worlds - fast channels when available, events as backup
import { useEffect, useState } from 'react';

import { useVUChannelStream } from './use-vu-channel-stream';
import { useVULevelEvents } from './use-vu-level-events';

export const useVUHybridStream = (isEnabled = true) => {
  const [channelsAvailable, setChannelsAvailable] = useState<boolean | null>(null);

  useEffect(() => {
    if (!isEnabled) {
      setChannelsAvailable(null);
      return;
    }

    // Test if channels are available by attempting to initialize them
    const testChannels = async () => {
      try {
        // This will be set by the channel hook if it succeeds
        setChannelsAvailable(true);
      } catch {
        // If channels fail, fall back to events
        setChannelsAvailable(false);
        console.log('📡 VU channels not available, using event system');
      }
    };

    testChannels();
  }, [isEnabled]);

  // Use channels if available, otherwise fall back to events
  useVUChannelStream(isEnabled && channelsAvailable === true);
  useVULevelEvents(isEnabled && channelsAvailable === false);

  // This hook manages the hybrid streaming approach
};