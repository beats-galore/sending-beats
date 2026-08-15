import type { Bus } from '../../../types/bus.types';
import { PortDot } from '../primitives/PortDot';
import { busInputPortOffset, busPortOffset } from './patch-geometry';

type BusPortsProps = {
  bus: Bus;
  /** Something is actually feeding the mix, so its outputs carry audio. */
  carrying: boolean;
};

/**
 * The patch points down a mix node's edges.
 *
 * They straddle the node's own bounds. Each input port is level with the member
 * row it belongs to, so a cable lands beside the source it carries and that
 * source's level; the outputs have no rows to line up with and are spread down
 * the edge instead.
 */
export const BusPorts = ({ bus, carrying }: BusPortsProps) => (
  <>
    {bus.inputs.map((deviceId, portIndex) => (
      <PortDot
        key={`in-${deviceId}`}
        tone="accent"
        side="left"
        top={busInputPortOffset(portIndex)}
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
