import {
  Group,
  NativeSelect,
  PasswordInput,
  SimpleGrid,
  Switch,
  Text,
  TextInput,
} from '@mantine/core';
import { useState } from 'react';

import {
  selectedCastConfiguration,
  useCastConfigurationStore,
} from '../../../stores/cast-configuration-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import type { CastConfigurationInput } from '../../../types/cast.types';
import { CAST_BITRATES, toInput } from '../../../types/cast.types';
import { ActionButton } from '../primitives/ActionButton';
import type { ControlDensity } from '../primitives/control-density';

type TransmitterControlsProps = {
  density: ControlDensity;
  isLive: boolean;
  isBusy: boolean;
  onToggle: () => void;
};

/**
 * The station the transmitter is pointed at, and the control that puts it on air.
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
  const station = useCastConfigurationStore(selectedCastConfiguration);
  const update = useCastConfigurationStore((state) => state.update);
  const setPassword = useCastConfigurationStore((state) => state.setPassword);

  // Held here rather than in the store: the stored password is in the keychain
  // and never comes back, so this field is only ever what is being typed now.
  const [passwordDraft, setPasswordDraft] = useState('');

  if (!station) {
    return (
      <Text size="xs" c={color.textFaint}>
        No station selected. Add one to say where the mix goes.
      </Text>
    );
  }

  const edit = (changes: Partial<CastConfigurationInput>) =>
    void update(station.id, { ...toInput(station), ...changes });

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
          value={station.serverHost}
          onChange={(event) => edit({ serverHost: event.currentTarget.value })}
          style={{ flex: 1, minWidth: 0 }}
          {...locked}
        />
        <TextInput
          value={String(station.serverPort)}
          onChange={(event) => edit({ serverPort: Number(event.currentTarget.value) || 0 })}
          w={74}
          {...locked}
        />
      </Group>

      <SimpleGrid cols={2} spacing={density === 'compact' ? 'lg' : 'xl'} verticalSpacing="lg">
        <TextInput
          label="MOUNT"
          value={station.mountPoint}
          onChange={(event) => edit({ mountPoint: event.currentTarget.value })}
          {...locked}
        />
        <NativeSelect
          label="BITRATE"
          value={String(station.bitrateKbps)}
          onChange={(event) => edit({ bitrateKbps: Number(event.currentTarget.value) })}
          data={CAST_BITRATES.map((rate) => ({ value: String(rate), label: `${rate} kbps` }))}
        />
        <TextInput
          label="USER"
          value={station.username}
          onChange={(event) => edit({ username: event.currentTarget.value })}
          {...locked}
        />
        {/* Typing replaces what is in the keychain; leaving it alone keeps it.
            The placeholder is the only report of what is stored, because the
            password itself never leaves the keychain. */}
        <PasswordInput
          label="PASSWORD"
          value={passwordDraft}
          placeholder={station.hasPassword ? '••••••••  stored' : 'not set'}
          onChange={(event) => setPasswordDraft(event.currentTarget.value)}
          onBlur={() => {
            if (passwordDraft.length > 0) {
              void setPassword(station.id, passwordDraft);
              setPasswordDraft('');
            }
          }}
          {...locked}
        />
      </SimpleGrid>

      <Group gap="md" wrap="nowrap">
        <Switch
          checked={station.variableBitrate}
          onChange={(event) => edit({ variableBitrate: event.currentTarget.checked })}
          label="Variable bitrate"
          style={{ flex: 1 }}
        />
        <Text size="2xs" c={color.textFaint}>
          {station.variableBitrate
            ? `quality ~V${station.vbrQuality}`
            : `constant ${station.bitrateKbps} kbps`}
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
