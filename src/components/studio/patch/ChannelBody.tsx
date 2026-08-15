import { Box, Group, NativeSelect, Stack, Text } from '@mantine/core';

import { color } from '../../../theme/tokens';
import type { AudioChannel } from '../../../types';
import { asGain, meterPosition } from '../format';
import type { useChannelSource } from '../hooks/use-channel-source';
import type { usePatchChannel } from '../hooks/use-patch-channel';
import { DragBar } from '../primitives/DragBar';
import { LevelMeter } from '../primitives/LevelMeter';
import { Pill } from '../primitives/Pill';
import { StatusDot } from '../primitives/StatusDot';
import { ChannelInspector } from './ChannelInspector';
import { DestinationTiles } from './DestinationTiles';
import { MuteSoloPills } from './MuteSoloPills';
import type { ChannelExpansion } from './patch-geometry';

const GAIN_MIN = -60;
const GAIN_MAX = 12;

type ChannelBodyProps = {
  channel: AudioChannel;
  /** Where this sits in the source column, for the port number on the inspector. */
  index: number;
  expansion: ChannelExpansion;
  patch: ReturnType<typeof usePatchChannel>;
  source: ReturnType<typeof useChannelSource>;
  /** The strip's own colour, which its meters read in. */
  meterBase: string;
};

/** A source at its ordinary size: what it is patched to, how loud, and where it goes. */
export const ChannelBody = ({
  channel,
  index,
  expansion,
  patch,
  source,
  meterBase,
}: ChannelBodyProps) => {
  const unavailable = source.unavailableReason !== null;
  const tone = unavailable ? 'hot' : source.isApplicationTap ? 'warn' : 'accent';
  const opened = expansion === 'inspector' || expansion === 'effects';

  return (
    <>
      <Group gap="xs" wrap="nowrap">
        <StatusDot
          tone={patch.muted && !unavailable ? 'inert' : tone}
          title={source.unavailableReason ?? undefined}
        />
        <NativeSelect
          value={source.configuredDevice?.deviceIdentifier ?? ''}
          onChange={(event) => void source.setSource(event.currentTarget.value)}
          onClick={(event) => event.stopPropagation()}
          data={[{ value: '', label: 'No input' }, ...source.options]}
          variant="unstyled"
          style={{ flex: 1, minWidth: 0 }}
          styles={{
            input: {
              color: unavailable ? color.hotText : color.textDim,
              fontSize: 'var(--mantine-font-size-xs)',
            },
          }}
        />
        {unavailable ? (
          <Pill tone="hot" size="3xs" title={source.unavailableReason ?? undefined}>
            OFFLINE
          </Pill>
        ) : (
          <Text size="3xs" c={color.textFaintest} style={{ flex: 'none' }}>
            {patch.isMono ? 'MONO' : 'STEREO'}
          </Text>
        )}
      </Group>

      <Stack gap="3xs">
        <LevelMeter
          level={meterPosition(patch.levels.left.peak)}
          dimmed={patch.muted}
          base={meterBase}
        />
        <LevelMeter
          level={meterPosition(patch.levels.right.peak)}
          dimmed={patch.muted}
          base={meterBase}
        />
      </Stack>

      <Group gap="sm" wrap="nowrap">
        <Text size="3xs" c={color.textFaint} w={12} style={{ flex: 'none' }}>
          {patch.isMono ? 'M' : 'LR'}
        </Text>
        <DragBar
          value={patch.gainDb}
          min={GAIN_MIN}
          max={GAIN_MAX}
          onChange={patch.setGain}
          tone={patch.muted ? 'muted' : 'accent'}
          knob={[10, 14]}
        />
        <Text size="2xs" w={46} ta="right" c={patch.muted ? color.textDim : color.acc}>
          {asGain(patch.gainDb)}
        </Text>
      </Group>

      <Group gap="xs" wrap="nowrap">
        {/* Sits in the space mute and solo already left, so showing where a
            source goes costs the card no extra height. */}
        <Box style={{ flex: 1, minWidth: 0 }}>
          <DestinationTiles deviceIdentifier={source.configuredDevice?.deviceIdentifier ?? null} />
        </Box>
        <MuteSoloPills
          muted={patch.muted}
          solo={patch.solo}
          onMute={patch.setMuted}
          onSolo={patch.setSolo}
        />
      </Group>

      {opened && (
        <ChannelInspector
          channel={channel}
          pan={patch.pan}
          onPanChange={patch.setPan}
          sourceName={patch.sourceName}
          port={index + 1}
          showChain={expansion === 'effects'}
        />
      )}
    </>
  );
};
