import { createContext, useContext } from 'react';

import type { PatchRects } from './patch-rects';

/**
 * Every node's resolved box, for the things that need to know about nodes other
 * than themselves.
 *
 * A drag has to hit-test the node being moved against all the others to work
 * out whether it is being dropped against an edge, and a node has no way to ask
 * about its neighbours. Threading the whole map down through five card
 * components to reach a hook would be worse than reaching for it here.
 */
const PatchRectsContext = createContext<PatchRects | null>(null);

export const PatchRectsProvider = PatchRectsContext.Provider;

/** Null outside the canvas, which is what tells a drag it has nothing to snap to. */
export const usePatchRectsContext = (): PatchRects | null => useContext(PatchRectsContext);
