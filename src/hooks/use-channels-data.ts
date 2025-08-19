// Hook specifically for channel data - only what ChannelGrid needs
import { useMixerStore } from '../stores';

export const useChannelsData = () => {
  // Only select the channels array, nothing else
  const channels = useMixerStore((state) => state.config?.channels || []);
  
  return {
    channels,
  };
};