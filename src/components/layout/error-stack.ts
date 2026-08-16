// Making a thrown error's stack readable during development.
//
// A React error in this app produces thirty-odd frames, of which about four are
// ours: the rest are the renderer walking its own work loop, served out of
// Vite's pre-bundled dependencies under names like `react-dom_client.js`. They
// are the same frames on every error and say nothing about this one.
//
// So the frames are split rather than filtered. What we wrote goes first, where
// it can be read at a glance; the rest is still there, because occasionally the
// error really is in a dependency and hiding that would send someone hunting
// through their own code for it.

/** A frame that came from this project rather than from something installed. */
const isOurs = (frame: string): boolean =>
  frame.includes('/src/') && !frame.includes('node_modules');

export type SplitStack = {
  /** Frames in this project, innermost first. */
  ours: string[];
  /** Everything else, kept for the case where the fault really is downstream. */
  theirs: string[];
};

export const splitStack = (stack: string | undefined): SplitStack => {
  const frames = (stack ?? '')
    .split('\n')
    .map((frame) => frame.trim())
    .filter((frame) => frame.length > 0);

  return {
    ours: frames.filter(isOurs),
    theirs: frames.filter((frame) => !isOurs(frame)),
  };
};

/**
 * Tidy up React's component stack.
 *
 * It arrives as lines of `    at ComponentName (url)`, which is the most useful
 * thing in the whole report — it says which components were rendering — but the
 * urls are long enough to push the names off the side.
 */
export const componentTrail = (componentStack: string | null | undefined): string[] =>
  (componentStack ?? '')
    .split('\n')
    .map((line) => line.trim().replace(/^at\s+/, ''))
    .map((line) => line.replace(/\s*\(.*\)$/, ''))
    .filter((line) => line.length > 0);
