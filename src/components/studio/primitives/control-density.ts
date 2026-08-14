/**
 * How much room a shared control set is given.
 *
 * The same controls appear inside a patchbay node and inside a full view. The
 * node has to fit them into a card, the view can let them breathe, and that
 * spacing is the only thing that differs between the two — so it is passed in
 * rather than being a reason to define the controls twice.
 */
export const ControlDensity = ['compact', 'roomy'] as const;
export type ControlDensity = (typeof ControlDensity)[number];
