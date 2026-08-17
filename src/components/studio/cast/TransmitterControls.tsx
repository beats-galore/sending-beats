import { Group, NativeSelect, PasswordInput, SimpleGrid, Switch, Text } from '@mantine/core';
import { useState } from 'react';

import {
  selectedCastConfiguration,
  useCastConfigurationStore,
} from '../../../stores/cast-configuration-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import {
  CAST_BITRATES,
  CAST_PROTOCOL_LABELS,
  CastProtocol,
  castSecretLabel,
} from '../../../types/cast.types';
import { ActionButton } from '../primitives/ActionButton';
import type { ControlDensity } from '../primitives/control-density';
import { IcecastFields } from './IcecastFields';
import { ImpulseFields } from './ImpulseFields';
import { useStationFields } from './use-station-fields';

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
 * The protocol chooses which address fields are shown, because the two have none
 * in common. Everything below that line is shared: both encode MP3 at a bitrate,
 * and both authenticate with one secret kept in the keychain.
 */
export const TransmitterControls = ({
  density,
  isLive,
  isBusy,
  onToggle,
}: TransmitterControlsProps) => {
  const selected = useCastConfigurationStore(selectedCastConfiguration);
  const setPassword = useCastConfigurationStore((state) => state.setPassword);

  // Held here rather than in the store: the stored secret is in the keychain
  // and never comes back, so this field is only ever what is being typed now.
  const [secretDraft, setSecretDraft] = useState('');

  const { station, edit, locked } = useStationFields(selected?.id ?? '', isLive);

  if (!station) {
    return (
      <Text size="xs" c={color.textFaint}>
        No station selected. Add one to say where the mix goes.
      </Text>
    );
  }

  const isImpulse = station.protocol === 'impulse';

  return (
    <>
      {/* Changing this changes what the other fields mean, so it comes first
          and is held while on air like the rest of the target. */}
      <NativeSelect
        label="CAST TYPE"
        value={station.protocol}
        onChange={(event) => edit({ protocol: event.currentTarget.value })}
        data={CastProtocol.map((protocol) => ({
          value: protocol,
          label: CAST_PROTOCOL_LABELS[protocol],
        }))}
        disabled={isLive}
      />

      {isImpulse ? (
        <ImpulseFields stationId={station.id} isLive={isLive} />
      ) : (
        <IcecastFields stationId={station.id} isLive={isLive} />
      )}

      <SimpleGrid cols={2} spacing={density === 'compact' ? 'lg' : 'xl'} verticalSpacing="lg">
        <NativeSelect
          label="BITRATE"
          value={String(station.bitrateKbps)}
          onChange={(event) => edit({ bitrateKbps: Number(event.currentTarget.value) })}
          data={CAST_BITRATES.map((rate) => ({ value: String(rate), label: `${rate} kbps` }))}
        />
        {/* Typing replaces what is in the keychain; leaving it alone keeps it.
            The placeholder is the only report of what is stored, because the
            secret itself never leaves the keychain. */}
        <PasswordInput
          label={castSecretLabel(station.protocol)}
          value={secretDraft}
          placeholder={station.hasPassword ? '••••••••  stored' : 'not set'}
          onChange={(event) => setSecretDraft(event.currentTarget.value)}
          onBlur={() => {
            if (secretDraft.length > 0) {
              void setPassword(station.id, secretDraft);
              setSecretDraft('');
            }
          }}
          {...locked}
        />
      </SimpleGrid>

      {/* Variable bitrate is an Icecast-only choice. Impulse measures each
          segment from the frames actually in it, so a varying rate would work —
          but the encoder is not set up for one, and offering the switch would
          promise something that does not happen. */}
      {!isImpulse && (
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
      )}

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
