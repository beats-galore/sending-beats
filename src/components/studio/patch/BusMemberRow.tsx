import { Box, Group, Text } from '@mantine/core';

import { useChannelLevels } from '../../../hooks';
import { channelTargetKey } from '../../../services/patch-color-service';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { meterPosition } from '../format';
import { usePatchColor } from '../hooks/use-patch-color';
import { LevelMeter } from '../primitives/LevelMeter';

type BusMemberRowProps = {
  channelId: number;
  /** Position in the source column, for the number and the colour. */
  index: number;
  name: string;
};

/**
 * One source feeding a mix: which one it is, and how hard it is pushing.
 *
 * The row sits level with the port its cable lands on, and reads in the
 * source's own colour, so what feeds a mix and how much each one contributes is
 * one horizontal glance rather than a row of names and a single summed meter.
 *
 * Read-only, like the tiles it replaces: routing is edited on the cards at
 * either end, and a third place to change it would leave the same edit
 * reachable three ways.
 */
export const BusMemberRow = ({ channelId, index, name }: BusMemberRowProps) => {
  const levels = useChannelLevels(channelId);
  const swatch = usePatchColor(channelTargetKey(channelId), index);

  // The louder side, so a source panned hard one way still reads as present
  // rather than as half a signal.
  const level = meterPosition(Math.max(levels.left.peak, levels.right.peak));
  const silent = level <= 0;

  return (
    <Group gap="sm" wrap="nowrap" h={layout.bus.memberRowHeight} style={{ flex: 'none' }}>
      <Box
        fz="3xs"
        style={{
          flex: 'none',
          padding: '1px 5px',
          borderRadius: 'var(--mantine-radius-xs)',
          fontWeight: 600,
          letterSpacing: layout.tracking.wide,
          background: silent ? color.dead : swatch.value,
          color: silent ? color.textDim : color.bg,
        }}
      >
        {String(index + 1).padStart(2, '0')}
      </Box>

      <Text
        size="xs"
        truncate
        c={silent ? color.textFaint : color.text}
        style={{ flex: '0 1 auto', minWidth: 0, maxWidth: layout.tile.maxWidth }}
        title={name}
      >
        {name}
      </Text>

      <Box style={{ flex: 1, minWidth: 0 }}>
        <LevelMeter level={level} surface="bgRaised" base={swatch.value} dimmed={silent} />
      </Box>
    </Group>
  );
};
