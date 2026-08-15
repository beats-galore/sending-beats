import { layout } from '../../../theme/layout';
import { NodeCard } from '../primitives/NodeCard';
import type { ChannelCardProps } from './channel-card';
import { channelHeight, channelWidth } from './patch-geometry';

/** A source node fed by hardware: the plain card, with nothing above the meters. */
export const DeviceCard = ({ expansion, top, children, ...card }: ChannelCardProps) => (
  <NodeCard
    {...card}
    position={{
      left: layout.source.x,
      top,
      width: channelWidth(expansion),
      height: channelHeight({ variant: 'device', expansion }),
    }}
    bodyStyle={{ display: 'flex', flexDirection: 'column', gap: 8, overflow: 'hidden' }}
  >
    {children}
  </NodeCard>
);
