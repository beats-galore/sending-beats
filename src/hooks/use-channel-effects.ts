// Custom hook for audio effects management on channels
import { useCallback } from 'react';

import { audioCalculations } from '../utils';
import { AUDIO } from '../utils/constants';

import { useMixerState } from './use-mixer-state';

export const useChannelEffects = (channelId: number) => {
  const { getChannelById, updateChannelEQ, updateChannelCompressor, updateChannelLimiter } =
    useMixerState();

  const channel = getChannelById(channelId);

  // EQ controls
  const setEQLowGain = useCallback(
    async (gain: number) => {
      const clampedGain = audioCalculations.clampGain(gain);
      await updateChannelEQ(channelId, { eq_low_gain: clampedGain });
    },
    [channelId, updateChannelEQ]
  );

  const setEQMidGain = useCallback(
    async (gain: number) => {
      const clampedGain = audioCalculations.clampGain(gain);
      await updateChannelEQ(channelId, { eq_mid_gain: clampedGain });
    },
    [channelId, updateChannelEQ]
  );

  const setEQHighGain = useCallback(
    async (gain: number) => {
      const clampedGain = audioCalculations.clampGain(gain);
      await updateChannelEQ(channelId, { eq_high_gain: clampedGain });
    },
    [channelId, updateChannelEQ]
  );

  const resetEQ = useCallback(async () => {
    await updateChannelEQ(channelId, {
      eq_low_gain: AUDIO.EQ_DEFAULT_GAIN,
      eq_mid_gain: AUDIO.EQ_DEFAULT_GAIN,
      eq_high_gain: AUDIO.EQ_DEFAULT_GAIN,
    });
  }, [channelId, updateChannelEQ]);

  // Compressor controls
  const setCompressorThreshold = useCallback(
    async (threshold: number) => {
      const clampedThreshold = Math.max(
        AUDIO.COMP_MIN_THRESHOLD,
        Math.min(AUDIO.COMP_MAX_THRESHOLD, threshold)
      );
      await updateChannelCompressor(channelId, { comp_threshold: clampedThreshold });
    },
    [channelId, updateChannelCompressor]
  );

  const setCompressorRatio = useCallback(
    async (ratio: number) => {
      const clampedRatio = Math.max(AUDIO.COMP_MIN_RATIO, Math.min(AUDIO.COMP_MAX_RATIO, ratio));
      await updateChannelCompressor(channelId, { comp_ratio: clampedRatio });
    },
    [channelId, updateChannelCompressor]
  );

  const setCompressorAttack = useCallback(
    async (attack: number) => {
      const clampedAttack = Math.max(
        AUDIO.COMP_MIN_ATTACK,
        Math.min(AUDIO.COMP_MAX_ATTACK, attack)
      );
      await updateChannelCompressor(channelId, { comp_attack: clampedAttack });
    },
    [channelId, updateChannelCompressor]
  );

  const setCompressorRelease = useCallback(
    async (release: number) => {
      const clampedRelease = Math.max(
        AUDIO.COMP_MIN_RELEASE,
        Math.min(AUDIO.COMP_MAX_RELEASE, release)
      );
      await updateChannelCompressor(channelId, { comp_release: clampedRelease });
    },
    [channelId, updateChannelCompressor]
  );

  const toggleCompressor = useCallback(async () => {
    if (channel) {
      await updateChannelCompressor(channelId, { comp_enabled: !channel.comp_enabled });
    }
  }, [channelId, channel, updateChannelCompressor]);

  const resetCompressor = useCallback(async () => {
    await updateChannelCompressor(channelId, {
      comp_threshold: AUDIO.COMP_DEFAULT_THRESHOLD,
      comp_ratio: AUDIO.COMP_DEFAULT_RATIO,
      comp_attack: AUDIO.COMP_DEFAULT_ATTACK,
      comp_release: AUDIO.COMP_DEFAULT_RELEASE,
      comp_enabled: false,
    });
  }, [channelId, updateChannelCompressor]);

  // Limiter controls
  const setLimiterThreshold = useCallback(
    async (threshold: number) => {
      const clampedThreshold = Math.max(
        AUDIO.LIMITER_MIN_THRESHOLD,
        Math.min(AUDIO.LIMITER_MAX_THRESHOLD, threshold)
      );
      await updateChannelLimiter(channelId, { limiter_threshold: clampedThreshold });
    },
    [channelId, updateChannelLimiter]
  );

  const toggleLimiter = useCallback(async () => {
    if (channel) {
      await updateChannelLimiter(channelId, { limiter_enabled: !channel.limiter_enabled });
    }
  }, [channelId, channel, updateChannelLimiter]);

  const resetLimiter = useCallback(async () => {
    await updateChannelLimiter(channelId, {
      limiter_threshold: AUDIO.LIMITER_DEFAULT_THRESHOLD,
      limiter_enabled: false,
    });
  }, [channelId, updateChannelLimiter]);

  // Reset all effects
  const resetAllEffects = useCallback(async () => {
    await Promise.all([resetEQ(), resetCompressor(), resetLimiter()]);
  }, [resetEQ, resetCompressor, resetLimiter]);

  // Get effect values with formatting
  const getEQValues = useCallback(() => {
    if (!channel) return null;

    return {
      low: {
        value: channel.eq_low_gain,
        display: audioCalculations.formatGain(channel.eq_low_gain),
      },
      mid: {
        value: channel.eq_mid_gain,
        display: audioCalculations.formatGain(channel.eq_mid_gain),
      },
      high: {
        value: channel.eq_high_gain,
        display: audioCalculations.formatGain(channel.eq_high_gain),
      },
    };
  }, [channel]);

  const getCompressorValues = useCallback(() => {
    if (!channel) return null;

    return {
      threshold: {
        value: channel.comp_threshold,
        display: audioCalculations.formatDb(channel.comp_threshold),
      },
      ratio: {
        value: channel.comp_ratio,
        display: `${channel.comp_ratio.toFixed(1)}:1`,
      },
      attack: {
        value: channel.comp_attack,
        display: `${channel.comp_attack.toFixed(1)} ms`,
      },
      release: {
        value: channel.comp_release,
        display: `${channel.comp_release.toFixed(0)} ms`,
      },
      enabled: channel.comp_enabled,
    };
  }, [channel]);

  const getLimiterValues = useCallback(() => {
    if (!channel) return null;

    return {
      threshold: {
        value: channel.limiter_threshold,
        display: audioCalculations.formatDb(channel.limiter_threshold),
      },
      enabled: channel.limiter_enabled,
    };
  }, [channel]);

  return {
    // EQ controls
    setEQLowGain,
    setEQMidGain,
    setEQHighGain,
    resetEQ,

    // Compressor controls
    setCompressorThreshold,
    setCompressorRatio,
    setCompressorAttack,
    setCompressorRelease,
    toggleCompressor,
    resetCompressor,

    // Limiter controls
    setLimiterThreshold,
    toggleLimiter,
    resetLimiter,

    // Combined actions
    resetAllEffects,

    // Value getters
    getEQValues,
    getCompressorValues,
    getLimiterValues,
  };
};
