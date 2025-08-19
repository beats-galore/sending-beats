// Hook for individual channel VU levels - only what each channel needs
import { useMixerStore } from '../stores';

export const useChannelLevels = (channelId: number) => {
  // Only select the specific channel's levels
  const channelLevels = useMixerStore((state) => {
    const channel = state.config?.channels.find(c => c.id === channelId);
    return {
      peak: channel?.peak_level || 0,
      rms: channel?.rms_level || 0,
    };
  });

  return channelLevels;
};