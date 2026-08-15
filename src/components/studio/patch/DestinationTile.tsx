import { Box } from '@mantine/core';

import { outputTargetKey } from '../../../services/patch-color-service';
import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import { usePatchColor } from '../hooks/use-patch-color';

type DestinationTileProps = {
  /** The destination this tile refers to */
  deviceId: string;
  /**
   * What its colour is stored against, when that is not the device itself.
   *
   * The broadcast and the tape reserve their colours rather than being given
   * one, so a tile pointing at either has to ask under the key that reservation
   * is held under.
   */
  targetKey?: string;
  name: string;
  /** Position in the destination column, for the number shown and the colour */
  index: number;
  /** Whether the source this tile sits on reaches that destination */
  on: boolean;
  onToggle: (deviceId: string) => void;
};

/**
 * One destination, on a source, saying whether the signal gets there.
 *
 * The mirror of `SourceTile`: painted in the destination's colour, because the
 * tile refers to the card on the far side of the canvas.
 */
export const DestinationTile = ({
  deviceId,
  targetKey, name, index, on, onToggle }: DestinationTileProps) => {
  const swatch = usePatchColor(targetKey ?? outputTargetKey(deviceId), index);

  return (
    <Box
      fz="3xs"
      onClick={(event) => {
        event.stopPropagation();
        onToggle(deviceId);
      }}
      title={`${name} · ${on ? 'receives this source' : 'does not receive this source'}`}
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
        cursor: 'pointer',
        border: on ? `1px solid ${swatch.value}` : border(),
        background: on ? swatch.value : undefined,
        color: on ? color.bg : color.textFaintest,
      }}
    >
      {String(index + 1).padStart(2, '0')} {name}
    </Box>
  );
};
