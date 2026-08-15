import { layout } from '../../../theme/layout';
import type { NowPlayingTrack } from '../../../types/now-playing.types';
import { NodeCard } from '../primitives/NodeCard';
import type { ChannelCardProps } from './channel-card';
import { ChannelTrack } from './ChannelTrack';
import { channelHeight, channelWidth } from './patch-geometry';

type AppCardProps = ChannelCardProps & {
  /** Null while the application is running but has nothing loaded. */
  track: NowPlayingTrack | null;
};

/**
 * A source node fed by an application, which reads what it is playing above the
 * meters. The row is held open even with nothing loaded so that starting and
 * stopping playback does not resize the card.
 */
export const AppCard = ({ expansion, top, track, children, ...card }: AppCardProps) => (
  <NodeCard
    {...card}
    position={{
      left: layout.source.x,
      top,
      width: channelWidth(expansion),
      height: channelHeight({ variant: 'app', expansion }),
    }}
    bodyStyle={{ display: 'flex', flexDirection: 'column', gap: 8, overflow: 'hidden' }}
  >
    <ChannelTrack track={track} />
    {children}
  </NodeCard>
);
