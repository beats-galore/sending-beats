// Audio calculation utilities for decibel conversions and level processing
import { AUDIO_CONSTANTS } from '../types';

// Decibel conversions
export const audioCalculations = {
  // Convert linear amplitude (0-1) to decibels
  linearToDb(linear: number): number {
    if (linear <= 0) {return AUDIO_CONSTANTS.MIN_DB;}
    return 20 * Math.log10(linear);
  },

  // Convert decibels to linear amplitude (0-1)
  dbToLinear(db: number): number {
    if (db <= AUDIO_CONSTANTS.MIN_DB) {return 0;}
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

  // Gain conversions
  gainDbToLinear(gainDb: number): number {
    return 10 ** (gainDb / 20);
  },

  gainLinearToDb(gainLinear: number): number {
    if (gainLinear <= 0) {return AUDIO_CONSTANTS.MIN_GAIN_DB;}
    return 20 * Math.log10(gainLinear);
  },

  // Pan calculations (-1 to +1)
  panToGains(pan: number): { left: number; right: number } {
    const clampedPan = Math.max(-1, Math.min(1, pan));
    const panRadians = (clampedPan * Math.PI) / 4; // -45° to +45°

    return {
      left: Math.cos(panRadians),
      right: Math.sin(panRadians),
    };
  },

  // Level processing
  calculateRms(samples: number[]): number {
    if (samples.length === 0) {return 0;}

    const squareSum = samples.reduce((sum, sample) => sum + sample * sample, 0);
    return Math.sqrt(squareSum / samples.length);
  },

  calculatePeak(samples: number[]): number {
    if (samples.length === 0) {return 0;}

    return Math.max(...samples.map(Math.abs));
  },

  // Apply gain to audio levels
  applyGain(level: number, gainDb: number): number {
    const gainLinear = this.gainDbToLinear(gainDb);
    return level * gainLinear;
  },

  // Compressor calculations
  calculateCompressionGain(inputDb: number, threshold: number, ratio: number): number {
    if (inputDb <= threshold) {return 0;} // No compression below threshold

    const overThreshold = inputDb - threshold;
    const compressedOver = overThreshold / ratio;

    return compressedOver - overThreshold; // Gain reduction in dB
  },

  // Limiter calculations
  calculateLimiterGain(inputDb: number, threshold: number): number {
    if (inputDb <= threshold) {return 0;}
    return threshold - inputDb; // Hard limiting
  },

  // EQ calculations (simplified)
  calculateEqGain(frequency: number, centerFreq: number, gain: number, q = 1): number {
    const freqRatio = frequency / centerFreq;
    const bandwidth = Math.log2(freqRatio) * q;
    const response = 10 ** (gain / 20 / (1 + bandwidth ** 2));

    return 20 * Math.log10(response);
  },

  // Utility functions
  clampDb(db: number, min = AUDIO_CONSTANTS.MIN_DB, max = AUDIO_CONSTANTS.MAX_DB): number {
    return Math.max(min, Math.min(max, db));
  },

  clampLinear(value: number, min = 0, max = 1): number {
    return Math.max(min, Math.min(max, value));
  },

  clampGain(gainDb: number): number {
    return Math.max(AUDIO_CONSTANTS.MIN_GAIN_DB, Math.min(AUDIO_CONSTANTS.MAX_GAIN_DB, gainDb));
  },

  clampPan(pan: number): number {
    return Math.max(AUDIO_CONSTANTS.MIN_PAN, Math.min(AUDIO_CONSTANTS.MAX_PAN, pan));
  },

  // Format dB values for display
  formatDb(db: number, decimals = 1): string {
    if (db <= AUDIO_CONSTANTS.MIN_DB) {return '-∞ dB';}
    return `${db.toFixed(decimals)} dB`;
  },

  // Format gain values for display
  formatGain(gainDb: number, decimals = 1): string {
    const sign = gainDb >= 0 ? '+' : '';
    return `${sign}${gainDb.toFixed(decimals)} dB`;
  },

  // Format pan values for display
  formatPan(pan: number): string {
    if (pan === 0) {return 'C';}
    if (pan < 0) {return `L${Math.abs(pan * 100).toFixed(0)}`;}
    return `R${(pan * 100).toFixed(0)}`;
  },
} as const;
