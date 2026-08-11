import {
  Group,
  NativeSelect,
  PasswordInput,
  SimpleGrid,
  Switch,
  Text,
  TextInput,
} from '@mantine/core';

import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { ActionButton } from '../primitives/ActionButton';
import { Panel } from '../primitives/Panel';
import { PanelHeading } from '../primitives/PanelHeading';
import { Pill } from '../primitives/Pill';

const BITRATES = [96, 128, 192, 256, 320];

type TransmitterPanelProps = {
  isLive: boolean;
  isBusy: boolean;
  onToggle: () => void;
};

/** The Icecast target and the control that puts it on air. */
export const TransmitterPanel = ({ isLive, isBusy, onToggle }: TransmitterPanelProps) => {
  const stream = useStudioStore((state) => state.stream);
  const setStream = useStudioStore((state) => state.setStream);

  return (
    <Panel
      p="4xl"
      gap="2xl"
      title={<PanelHeading order={2}>THE TRANSMITTER</PanelHeading>}
      action={
        <Pill tone={isLive ? 'accent' : 'neutral'} filled={isLive} size="2xs">
          {isLive ? 'CONNECTED' : 'DISCONNECTED'}
        </Pill>
      }
    >
      <Group gap="sm" wrap="nowrap" align="flex-end">
        <TextInput
          label="SERVER"
          value={stream.host}
          onChange={(event) => setStream({ host: event.currentTarget.value })}
          style={{ flex: 1, minWidth: 0 }}
        />
        <TextInput
          value={String(stream.port)}
          onChange={(event) => setStream({ port: Number(event.currentTarget.value) || 0 })}
          w={74}
        />
      </Group>

      <SimpleGrid cols={2} spacing="xl" verticalSpacing="lg">
        <TextInput
          label="MOUNT"
          value={stream.mount}
          onChange={(event) => setStream({ mount: event.currentTarget.value })}
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
        />
        <PasswordInput
          label="PASSWORD"
          value={stream.password}
          onChange={(event) => setStream({ password: event.currentTarget.value })}
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
        size="lg"
        padding="13px 0"
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
    </Panel>
  );
};
