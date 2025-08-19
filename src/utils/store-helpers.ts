// Store utilities for optimized state updates
import isEqual from 'fast-deep-equal';

/**
 * Helper to update state only if values have actually changed
 * Prevents unnecessary re-renders by preserving object references
 */
export const createOptimizedUpdater = <T>(setValue: (updater: (state: T) => Partial<T>) => void) => {
  return (updater: (state: T) => Partial<T>) => {
    setValue((state) => {
      const updates = updater(state);
      
      // Check if any values actually changed
      const hasChanges = Object.entries(updates).some(([key, newValue]) => {
        const currentValue = (state as any)[key];
        return !isEqual(currentValue, newValue);
      });
      
      // Only return updates if there are actual changes
      return hasChanges ? updates : {};
    });
  };
};

/**
 * Update array items only if they've changed
 * Preserves references for unchanged items
 */
export const updateArrayItems = <T>(
  array: T[],
  updateFn: (item: T, index: number) => T,
  compareFn?: (oldItem: T, newItem: T) => boolean
): T[] => {
  let hasChanges = false;
  const compare = compareFn || isEqual;
  
  const newArray = array.map((item, index) => {
    const updatedItem = updateFn(item, index);
    if (!compare(item, updatedItem)) {
      hasChanges = true;
      return updatedItem;
    }
    return item; // Return same reference if unchanged
  });
  
  // Return same array reference if no changes
  return hasChanges ? newArray : array;
};

/**
 * Shallow comparison for objects with known structure
 * More efficient than deep equality for simple objects
 */
export const shallowEqual = <T extends Record<string, any>>(obj1: T, obj2: T): boolean => {
  const keys1 = Object.keys(obj1);
  const keys2 = Object.keys(obj2);
  
  if (keys1.length !== keys2.length) return false;
  
  for (const key of keys1) {
    if (obj1[key] !== obj2[key]) return false;
  }
  
  return true;
};

/**
 * Check if nested object properties have changed
 * Useful for level updates in audio contexts
 */
export const hasLevelChanges = (
  oldLevels: { peak_level: number; rms_level: number },
  newLevels: { peak_level: number; rms_level: number }
): boolean => {
  return oldLevels.peak_level !== newLevels.peak_level || 
         oldLevels.rms_level !== newLevels.rms_level;
};