// Hook for individual channel VU levels - stereo format [peak_left, rms_left, peak_right, rms_right]
import { useMixerStore } from '../stores';

export const useChannelLevels = (channelId: number) => {
  // Get stereo channel levels from VU meter data
  const channelLevels = useMixerStore((state) => {
    const levelData = state.channelLevels?.[channelId];
    
    if (levelData && Array.isArray(levelData) && levelData.length === 4) {
      // Stereo format: [peak_left, rms_left, peak_right, rms_right]
      return {
        left: {
          peak: levelData[0],
          rms: levelData[1],
        },
        right: {
          peak: levelData[2],
          rms: levelData[3],
        },
        // Legacy mono values (average of L/R for compatibility)
        peak: (levelData[0] + levelData[2]) / 2,
        rms: (levelData[1] + levelData[3]) / 2,
      };
    }
    
    // Fallback for missing data
    return {
      left: { peak: 0, rms: 0 },
      right: { peak: 0, rms: 0 },
      peak: 0,
      rms: 0,
    };
  });

  return channelLevels;
};
