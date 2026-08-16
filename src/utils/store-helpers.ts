// Store utilities for optimized state updates
import isEqual from 'fast-deep-equal';

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
