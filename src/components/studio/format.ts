// Display formatting shared across the studio views.

const pad = (value: number): string => String(Math.floor(value)).padStart(2, '0');

/** Seconds as HH:MM:SS — transport counters. */
export const asClock = (seconds: number): string =>
  `${pad(seconds / 3600)}:${pad((seconds / 60) % 60)}:${pad(seconds % 60)}`;

/** Seconds as HH:MM — durations and elapsed readouts. */
export const asElapsed = (seconds: number): string =>
  `${pad(seconds / 3600)}:${pad((seconds / 60) % 60)}`;

/** A signed decibel reading, e.g. `+2.4dB`. */
export const asGain = (db: number, decimals = 1): string =>
  `${db > 0 ? '+' : ''}${db.toFixed(decimals)}dB`;

/** A pan position as `C`, `L42` or `R28`. */
export const asPan = (pan: number): string => {
  if (Math.abs(pan) < 0.02) {
    return 'C';
  }
  return `${pan < 0 ? 'L' : 'R'}${Math.round(Math.abs(pan) * 100)}`;
};

export const asBytes = (bytes: number): string => {
  const mb = bytes / 1_000_000;
  return mb >= 1000 ? `${(mb / 1000).toFixed(2)} GB` : `${mb.toFixed(1)} MB`;
};

/** Linear amplitude to decibels, floored so silence reads as the bottom of the scale. */
export const linearToDb = (linear: number, floor = -60): number =>
  linear <= 0 ? floor : Math.max(floor, 20 * Math.log10(linear));

export const dbToLinear = (db: number): number => 10 ** (db / 20);

/**
 * Normalises a linear level onto the meter scale.
 *
 * Meters are read in dB, not amplitude — mapping linearly would leave everything
 * below half scale bunched against the left edge.
 */
export const meterPosition = (linear: number, floorDb = -60): number =>
  Math.min(1, Math.max(0, (linearToDb(linear, floorDb) - floorDb) / -floorDb));
