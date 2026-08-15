import { Group, Stack } from '@mantine/core';

import { layout } from '../../../theme/layout';
import { meterPosition } from '../format';
import type { usePatchChannel } from '../hooks/use-patch-channel';
import { LevelMeter } from '../primitives/LevelMeter';
import { MuteSoloPills } from './MuteSoloPills';

type ChannelCompactProps = {
  patch: ReturnType<typeof usePatchChannel>;
};

/**
 * A source shrunk as far as it goes: whether it is making noise, and the two
 * flags that stop it.
 *
 * The name is already in the title bar, so what is left is the pair of meters —
 * thinned down so a column of shrunk sources reads as a set of levels rather
 * than a stack of cards — and mute and solo beside them. Everything else about
 * a source can be read once it is opened; these cannot wait that long.
 */
export const ChannelCompact = ({ patch }: ChannelCompactProps) => (
  <Group gap="xs" wrap="nowrap" style={{ flex: 1, minHeight: 0 }} align="center">
    <Stack gap="3xs" style={{ flex: 1, minWidth: 0 }}>
      <LevelMeter
        level={meterPosition(patch.levels.left.peak)}
        height={layout.compactMeterHeight}
        dimmed={patch.muted}
      />
      <LevelMeter
        level={meterPosition(patch.levels.right.peak)}
        height={layout.compactMeterHeight}
        dimmed={patch.muted}
      />
    </Stack>
    <MuteSoloPills
      muted={patch.muted}
      solo={patch.solo}
      onMute={patch.setMuted}
      onSolo={patch.setSolo}
    />
  </Group>
);
