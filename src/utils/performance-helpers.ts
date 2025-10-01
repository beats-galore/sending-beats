// Performance optimization utilities for React components
import { useCallback, useMemo, useRef } from 'react';

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

// Throttle hook for real-time updates
export const useThrottle = <T extends (...args: any[]) => void>(callback: T, delay: number): T => {
  const lastRun = useRef(Date.now());

  return useCallback(
    ((...args: any[]) => {
      if (Date.now() - lastRun.current >= delay) {
        callback(...args);
        lastRun.current = Date.now();
      }
    }) as T,
    [callback, delay]
  );
};

// Memoized comparison for shallow objects
export const useShallowMemo = <T>(factory: () => T, deps: React.DependencyList): T => {
  return useMemo(factory, deps);
};

// Deep comparison for complex objects (use sparingly)
export const useDeepMemo = <T>(factory: () => T, deps: React.DependencyList): T => {
  const depsRef = useRef<React.DependencyList>();
  const valueRef = useRef<T>();

  // Simple deep equality check for deps
  const depsEqual =
    depsRef.current &&
    deps.length === depsRef.current.length &&
    deps.every((dep, index) => JSON.stringify(dep) === JSON.stringify(depsRef.current![index]));

  if (!depsEqual) {
    depsRef.current = deps;
    valueRef.current = factory();
  }

  return valueRef.current!;
};

// Optimized array comparison for VU meter updates
export const arraysEqual = <T>(a: T[], b: T[]): boolean => {
  if (a.length !== b.length) {return false;}

  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) {return false;}
  }

  return true;
};

// Optimized object comparison for audio settings
export const shallowEqual = <T extends Record<string, any>>(objA: T, objB: T): boolean => {
  const keysA = Object.keys(objA);
  const keysB = Object.keys(objB);

  if (keysA.length !== keysB.length) {return false;}

  for (const key of keysA) {
    if (objA[key] !== objB[key]) {return false;}
  }

  return true;
};

// Performance monitoring utility
export const withPerformanceLogging = <T extends (...args: any[]) => any>(
  fn: T,
  name: string
): T => {
  return ((...args: any[]) => {
    const start = performance.now();
    const result = fn(...args);
    const end = performance.now();

    if (end - start > 16) {
      // Log if takes more than one frame (16ms at 60fps)
      console.warn(`Performance: ${name} took ${(end - start).toFixed(2)}ms`);
    }

    return result;
  }) as T;
};

// Batch state updates to prevent multiple re-renders
export const batchUpdates = (callback: () => void) => {
  // In React 18+, updates are automatically batched
  // But we can still wrap in setTimeout for older versions
  if (typeof window !== 'undefined' && 'requestAnimationFrame' in window) {
    requestAnimationFrame(callback);
  } else {
    setTimeout(callback, 0);
  }
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
