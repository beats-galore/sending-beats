import { Box } from '@mantine/core';

import type { PatchTargetKey } from '../../../services/patch-color-service';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { usePatchColor } from '../hooks/use-patch-color';

type MemberTileProps = {
  targetKey: PatchTargetKey;
  /** Position in the column the member belongs to, for its number and colour */
  position: number;
  label: string;
};

/**
 * A member of a bus, named and in its own colour.
 *
 * Read-only: routing is edited on the cards at either end, and offering a third
 * place to change it would leave the same edit reachable three ways.
 */
export const MemberTile = ({ targetKey, position, label }: MemberTileProps) => {
  const swatch = usePatchColor(targetKey, position);

  return (
    <Box
      fz="3xs"
      title={label}
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
        border: `1px solid ${swatch.value}`,
        background: swatch.value,
        color: color.bg,
      }}
    >
      {String(position + 1).padStart(2, '0')} {label}
    </Box>
  );
};
