import { NodeCard } from '../primitives/NodeCard';
import type { ChannelCardProps } from './channel-card';

/** A source node fed by hardware: the plain card, with nothing above the meters. */
export const DeviceCard = ({ rect, children, ...card }: ChannelCardProps) => (
  <NodeCard
    {...card}
    position={rect}
    bodyStyle={{ display: 'flex', flexDirection: 'column', gap: 8, overflow: 'hidden' }}
  >
    {children}
  </NodeCard>
);
