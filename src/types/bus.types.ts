/**
 * The mix a device belongs to until it is routed somewhere else.
 *
 * The registry keeps it whatever happens to it — `drop_unheard_buses` spares it
 * by id — so the canvas can rely on it being there and pin it to the top.
 */
export const MAIN_BUS_ID = 'main';

/**
 * A named mix, its members, and the trim applied to it.
 *
 * Membership is by device identifier — the string the mixing layer routes by —
 * not by the row id of a configured device, which is recreated whenever a
 * channel's source is switched.
 */
export type Bus = {
  id: string;
  name: string;
  gain: number;
  /** Input device identifiers summed into this bus */
  inputs: string[];
  /** Output device identifiers that receive this bus */
  outputs: string[];
};
