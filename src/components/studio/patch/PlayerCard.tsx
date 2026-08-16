import type { useFilePlayer } from '../hooks/use-file-player';
import { NodeCard } from '../primitives/NodeCard';
import type { ChannelCardProps } from './channel-card';
import { PlayerTransport } from './PlayerTransport';

type PlayerCardProps = ChannelCardProps & {
  player: ReturnType<typeof useFilePlayer>;
  /** The strip's own colour, which the transport reads in. */
  tint: string;
};

/**
 * A source node fed by a queue of files, which carries its transport above the
 * meters. The row is held open with nothing queued, so loading the first file
 * does not resize the card.
 */
export const PlayerCard = ({ rect, player, tint, children, ...card }: PlayerCardProps) => (
  <NodeCard
    {...card}
    position={rect}
    bodyStyle={{ display: 'flex', flexDirection: 'column', gap: 8, overflow: 'hidden' }}
  >
    <PlayerTransport player={player} tint={tint} />
    {children}
  </NodeCard>
);
