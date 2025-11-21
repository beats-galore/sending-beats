// Performance optimization utilities for React components
import { useCallback, useRef } from 'react';

// Debounce hook for audio parameter updates
export const useDebounce = <T extends (...args: any[]) => void>(callback: T, delay: number): T => {
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>();

  return useCallback(
    ((...args: any[]) => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }

      timeoutRef.current = setTimeout(() => {
        callback(...args);
      }, delay);
    }) as T,
    [callback, delay]
  );
};

// VU meter specific performance optimizations
export const VU_METER_OPTIMIZATIONS = {
  // Threshold for skipping re-renders (0.1% change)
  RENDER_THRESHOLD: 0.001,

  // Pre-calculated segment colors to avoid repeated calculations
  SEGMENT_COLORS: {
    GREEN: '#10b981',
    YELLOW: '#f59e0b',
    RED: '#ef4444',
    OFF: '#4b5563',
  },

  // Create memoized level comparison function
  levelsEqual: (
    a: { peak: number; rms: number },
    b: { peak: number; rms: number },
    threshold = 0.001
  ): boolean => {
    return Math.abs(a.peak - b.peak) < threshold && Math.abs(a.rms - b.rms) < threshold;
  },

  // Optimized dB conversion with caching
  createDbCache: (maxSize = 1000) => {
    const cache = new Map<number, number>();
    return (value: number): number => {
      if (!cache.has(value)) {
        // Keep cache size under control
        if (cache.size >= maxSize) {
          const firstKey = cache.keys().next().value;
          if (firstKey !== undefined) {
            cache.delete(firstKey);
          }
        }
        cache.set(value, value > 0 ? 20 * Math.log10(value) : -60);
      }
      return cache.get(value)!;
    };
  },
} as const;
