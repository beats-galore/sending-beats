import { channelTargetKey } from '../../../services/patch-color-service';
import { useFilePlayerStore } from '../../../stores/file-player-store';
import { useConfigurationStore } from '../../../stores/mixer-store';
import { patchedPlayerId } from '../../../types/file-player.types';
import { useFocusedNode } from '../hooks/use-focused-node';
import { usePatchColor } from '../hooks/use-patch-color';
import type { PatchRects } from './patch-rects';
import { QueuePanel } from './QueuePanel';

type QueueOverlayProps = {
  /** In column order, matching `rects.channels`. */
  channelIds: number[];
  rects: PatchRects;
};

/**
 * The queue for whichever player is selected, if one is.
 *
 * Selecting a source is how you say you are working on it, and for a player
 * that means the queue. Nothing else on the canvas has anything this size to
 * reveal, so it is a panel beside the node rather than part of the card.
 */
export const QueueOverlay = ({ channelIds, rects }: QueueOverlayProps) => {
  const focused = useFocusedNode();
  const players = useFilePlayerStore((state) => state.players);
  const { activeSession } = useConfigurationStore();

  const channelId = focused?.kind === 'channel' ? focused.channelId : null;
  const index = channelId === null ? -1 : channelIds.indexOf(channelId);

  const device = (activeSession?.configuredDevices ?? []).find(
    (entry) => entry.isInput && entry.channelNumber === channelId
  );
  const playerId = patchedPlayerId(device?.deviceIdentifier, players);

  // Called unconditionally: the selected node's colour is what the panel reads
  // in, and hooks cannot be skipped for the case where it is not a player.
  const swatch = usePatchColor(channelTargetKey(channelId ?? 0), Math.max(index, 0));

  if (!playerId || index === -1) {
    return null;
  }

  return <QueuePanel playerId={playerId} anchor={rects.channels[index]} tint={swatch.value} />;
};
