import { useFilePlayerStore } from '../../../stores/file-player-store';
import { useMixerStore } from '../../../stores/mixer-store';
import { patchedPlayerId } from '../../../types/file-player.types';
import { expansionFor } from './patch-geometry';
import type { ChannelCardVariant } from './patch-geometry';
import type { PatchRects } from './patch-rects';
import { QueuePanel } from './QueuePanel';

type QueueOverlayProps = {
  /** In column order, matching `rects.channels`. */
  channels: { id: number; variant: ChannelCardVariant }[];
  rects: PatchRects;
};

/**
 * The queue beside every player that has room to show one.
 *
 * A companion to the card rather than something you go and open: a queue is
 * what a player is for, so it stands next to the card for as long as the card
 * is showing. Shrinking the card to its meters is what puts the queue away,
 * which is the same gesture that puts the rest of the card away.
 */
export const QueueOverlay = ({ channels, rects }: QueueOverlayProps) => {
  const players = useFilePlayerStore((state) => state.players);
  const activeSession = useMixerStore((state) => state.activeSession);

  const panels = channels
    .map((channel, index) => ({ channel, index, anchor: rects.channels[index] }))
    .filter(({ channel, anchor }) => {
      if (channel.variant !== 'player' || !anchor) {
        return false;
      }
      return expansionFor(channel.variant, anchor) !== 'compact';
    })
    .map(({ channel, index, anchor }) => {
      const device = (activeSession?.configuredDevices ?? []).find(
        (entry) => entry.isInput && entry.channelNumber === channel.id
      );

      return {
        channelId: channel.id,
        index,
        anchor,
        playerId: patchedPlayerId(device?.deviceIdentifier, players),
      };
    });

  return (
    <>
      {panels.map(
        (panel) =>
          panel.playerId && (
            <QueuePanel
              key={panel.channelId}
              playerId={panel.playerId}
              channelId={panel.channelId}
              position={panel.index}
              anchor={panel.anchor}
            />
          )
      )}
    </>
  );
};
