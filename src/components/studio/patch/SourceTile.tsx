import { Box } from '@mantine/core';

import { channelTargetKey } from '../../../services/patch-color-service';
import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import { useChannelSource } from '../hooks/use-channel-source';
import { usePatchColor } from '../hooks/use-patch-color';

type SourceTileProps = {
  /** The channel this tile refers to, which is also what it is coloured by */
  channelId: number;
  /** Position in the source column, for the number shown and the colour */
  index: number;
  /** The channel's stored name, empty when it has not been named */
  name: string;
  /**
   * Device identifiers reaching the destination this tile sits on.
   *
   * Membership is decided here rather than by the caller because the identifier
   * a channel routes by is not on the channel — it is on the configured device
   * patched into it, which this already has to resolve to be clickable.
   */
  sources: string[];
  onToggle: (deviceIdentifier: string) => void;
};

/**
 * One source, on a destination, saying whether it gets there.
 *
 * Painted in the source's colour rather than the destination's — the tile is a
 * reference to the strip on the far side of the canvas, and matching it is what
 * makes the routing readable without following a cable.
 */
export const SourceTile = ({ channelId, index, name, sources, onToggle }: SourceTileProps) => {
  const source = useChannelSource(channelId);
  const swatch = usePatchColor(channelTargetKey(channelId), index);

  const deviceIdentifier = source.configuredDevice?.deviceIdentifier ?? null;
  const patched = deviceIdentifier !== null;
  const on = deviceIdentifier !== null && sources.includes(deviceIdentifier);

  // An unnamed channel borrows what is patched into it, the same fallback the
  // strip's own title uses, so a tile and the card it points at read alike.
  const label = name || source.configuredDevice?.deviceName || 'No input';

  return (
    <Box
      fz="3xs"
      onClick={(event) => {
        event.stopPropagation();
        if (deviceIdentifier) {
          onToggle(deviceIdentifier);
        }
      }}
      title={
        patched
          ? `${label} · ${on ? 'reaches this' : 'does not reach this'}`
          : 'Patch a source into this channel first'
      }
      style={{
        flex: 'none',
        maxWidth: layout.tile.maxWidth,
        overflow: 'hidden',
        textOverflow: 'ellipsis',
        whiteSpace: 'nowrap',
        padding: '2px 6px',
        borderRadius: 'var(--mantine-radius-xs)',
        fontWeight: 600,
        letterSpacing: layout.tracking.wide,
        // A channel with no source cannot be routed anywhere, so it reads as
        // unavailable rather than merely off.
        cursor: patched ? 'pointer' : 'default',
        opacity: patched ? 1 : 0.4,
        border: on ? `1px solid ${swatch.value}` : border(),
        background: on ? swatch.value : undefined,
        color: on ? color.bg : color.textFaintest,
      }}
    >
      {String(index + 1).padStart(2, '0')} {label}
    </Box>
  );
};
