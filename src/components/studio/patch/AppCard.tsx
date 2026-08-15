import type { NowPlayingTrack } from '../../../types/now-playing.types';
import { NodeCard } from '../primitives/NodeCard';
import type { ChannelCardProps } from './channel-card';
import { ChannelTrack } from './ChannelTrack';

type AppCardProps = ChannelCardProps & {
  /** Null while the application is running but has nothing loaded. */
  track: NowPlayingTrack | null;
};

/**
 * A source node fed by an application, which reads what it is playing above the
 * meters. The row is held open even with nothing loaded so that starting and
 * stopping playback does not resize the card.
 */
export const AppCard = ({ rect, track, children, ...card }: AppCardProps) => (
  <NodeCard
    {...card}
    position={rect}
    bodyStyle={{ display: 'flex', flexDirection: 'column', gap: 8, overflow: 'hidden' }}
  >
    <ChannelTrack track={track} />
    {children}
  </NodeCard>
);
