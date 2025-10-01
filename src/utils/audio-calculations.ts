// Audio calculation utilities for decibel conversions and level processing
import { AUDIO_CONSTANTS } from '../types';

// Decibel conversions
export const audioCalculations = {
  // Convert linear amplitude (0-1) to decibels
  linearToDb(linear: number): number {
    if (linear <= 0) {
      return AUDIO_CONSTANTS.MIN_DB;
    }
    return 20 * Math.log10(linear);
  },

  // Convert decibels to linear amplitude (0-1)
  dbToLinear(db: number): number {
    if (db <= AUDIO_CONSTANTS.MIN_DB) {
      return 0;
    }
    return 10 ** (db / 20);
  },

  // Convert dB to VU meter position (0-1 range)
  dbToVuPosition(db: number): number {
    return Math.max(
      0,
      Math.min(1, (db - AUDIO_CONSTANTS.MIN_DB) / (AUDIO_CONSTANTS.MAX_DB - AUDIO_CONSTANTS.MIN_DB))
    );
  },

  // Convert VU meter position (0-1) back to dB
  vuPositionToDb(position: number): number {
    const clampedPosition = Math.max(0, Math.min(1, position));
    return (
      AUDIO_CONSTANTS.MIN_DB + clampedPosition * (AUDIO_CONSTANTS.MAX_DB - AUDIO_CONSTANTS.MIN_DB)
    );
  },

  clampGain(gainDb: number): number {
    return Math.max(AUDIO_CONSTANTS.MIN_GAIN_DB, Math.min(AUDIO_CONSTANTS.MAX_GAIN_DB, gainDb));
  },

  // Format dB values for display
  formatDb(db: number, decimals = 1): string {
    if (db <= AUDIO_CONSTANTS.MIN_DB) {
      return '-∞ dB';
    }
    return `${db.toFixed(decimals)} dB`;
  },

  // Format gain values for display
  formatGain(gainDb: number, decimals = 1): string {
    const sign = gainDb >= 0 ? '+' : '';
    return `${sign}${gainDb.toFixed(decimals)} dB`;
  },
} as const;
