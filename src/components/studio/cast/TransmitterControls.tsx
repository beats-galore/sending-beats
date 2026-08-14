import { Group, NativeSelect, PasswordInput, SimpleGrid, Switch, Text, TextInput } from '@mantine/core';

import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { ActionButton } from '../primitives/ActionButton';
import type { ControlDensity } from '../primitives/control-density';

const BITRATES = [96, 128, 192, 256, 320];

type TransmitterControlsProps = {
  density: ControlDensity;
  isLive: boolean;
  isBusy: boolean;
  onToggle: () => void;
};

/**
 * The Icecast target and the control that puts it on air.
 *
 * Rows only — the caller supplies the surface and the gap between them, which
 * is what separates the patchbay's cast node from the CAST view.
 *
 * Going live hands the target to the engine, so the connection fields are read
 * only while on air. Bitrate and variable bitrate stay editable — they can be
 * changed without tearing the connection down.
 */
export const TransmitterControls = ({
  density,
  isLive,
  isBusy,
  onToggle,
}: TransmitterControlsProps) => {
  const stream = useStudioStore((state) => state.stream);
  const setStream = useStudioStore((state) => state.setStream);

  // Read-only inputs keep their layout but read as settled rather than editable,
  // so a live target does not look like a field waiting for input.
  const locked = {
    readOnly: isLive,
    styles: isLive ? { input: { color: color.textDim, cursor: 'default' } } : undefined,
  };

  return (
    <>
      <Group gap="sm" wrap="nowrap" align="flex-end">
        <TextInput
          label="SERVER"
          value={stream.host}
          onChange={(event) => setStream({ host: event.currentTarget.value })}
          style={{ flex: 1, minWidth: 0 }}
          {...locked}
        />
        <TextInput
          value={String(stream.port)}
          onChange={(event) => setStream({ port: Number(event.currentTarget.value) || 0 })}
          w={74}
          {...locked}
        />
      </Group>

      <SimpleGrid cols={2} spacing={density === 'compact' ? 'lg' : 'xl'} verticalSpacing="lg">
        <TextInput
          label="MOUNT"
          value={stream.mount}
          onChange={(event) => setStream({ mount: event.currentTarget.value })}
          {...locked}
        />
        <NativeSelect
          label="BITRATE"
          value={String(stream.bitrate)}
          onChange={(event) => setStream({ bitrate: Number(event.currentTarget.value) })}
          data={BITRATES.map((rate) => ({ value: String(rate), label: `${rate} kbps` }))}
        />
        <TextInput
          label="USER"
          value={stream.username}
          onChange={(event) => setStream({ username: event.currentTarget.value })}
          {...locked}
        />
        <PasswordInput
          label="PASSWORD"
          value={stream.password}
          onChange={(event) => setStream({ password: event.currentTarget.value })}
          {...locked}
        />
      </SimpleGrid>

      <Group gap="md" wrap="nowrap">
        <Switch
          checked={stream.variableBitrate}
          onChange={(event) => setStream({ variableBitrate: event.currentTarget.checked })}
          label="Variable bitrate"
          style={{ flex: 1 }}
        />
        <Text size="2xs" c={color.textFaint}>
          {stream.variableBitrate
            ? `quality ~V${stream.vbrQuality}`
            : `constant ${stream.bitrate} kbps`}
        </Text>
      </Group>

      <ActionButton
        tone={isLive ? 'hot' : 'accent'}
        fullWidth
        size={density === 'compact' ? '2xs' : 'lg'}
        padding={density === 'compact' ? '10px 0' : '13px 0'}
        disabled={isBusy}
        onClick={onToggle}
      >
        <span
          style={{
            fontFamily: 'var(--mantine-font-family-headings)',
            letterSpacing: layout.tracking.caps,
          }}
        >
          {isLive ? 'CUT THE FEED' : 'GO LIVE'}
        </span>
      </ActionButton>
    </>
  );
};
