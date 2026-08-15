import type { Bus } from '../../../types/bus.types';
import { PortDot } from '../primitives/PortDot';
import { busPortOffset } from './patch-geometry';

type BusPortsProps = {
  bus: Bus;
  /** Something is actually feeding the mix, so its outputs carry audio. */
  carrying: boolean;
};

/**
 * The patch points down a mix node's edges.
 *
 * They straddle the node's own bounds, and are listed in the order the member
 * tiles are, so a cable lands beside the tile naming what it carries.
 */
export const BusPorts = ({ bus, carrying }: BusPortsProps) => (
  <>
    {bus.inputs.map((deviceId, portIndex) => (
      <PortDot
        key={`in-${deviceId}`}
        tone="accent"
        side="left"
        top={busPortOffset(portIndex, bus.inputs.length)}
      />
    ))}
    {bus.outputs.map((deviceId, portIndex) => (
      <PortDot
        key={`out-${deviceId}`}
        tone={carrying ? 'accent' : 'dead'}
        side="right"
        top={busPortOffset(portIndex, bus.outputs.length)}
      />
    ))}
  </>
);
