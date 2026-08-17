import { Box, Group, ScrollArea, Text, TextInput } from '@mantine/core';

import {
  selectedCastConfiguration,
  useCastConfigurationStore,
} from '../../../stores/cast-configuration-store';
import { border, color } from '../../../theme/tokens';
import { castAddress, toInput } from '../../../types/cast.types';
import { ActionButton } from '../primitives/ActionButton';
import { DeleteButton } from '../primitives/DeleteButton';
import { Panel } from '../primitives/Panel';
import { PanelHeading } from '../primitives/PanelHeading';
import { SectionLabel } from '../primitives/SectionLabel';
import { StatusDot } from '../primitives/StatusDot';

type StationPickerProps = {
  /** Switching station mid-broadcast would cut the feed, so it is held while live */
  isLive: boolean;
};

/** The places this studio broadcasts to, and which one the transmitter is on. */
export const StationPicker = ({ isLive }: StationPickerProps) => {
  const configurations = useCastConfigurationStore((state) => state.configurations);
  const selectedId = useCastConfigurationStore((state) => state.selectedId);
  const select = useCastConfigurationStore((state) => state.select);
  const add = useCastConfigurationStore((state) => state.add);
  const remove = useCastConfigurationStore((state) => state.remove);
  const update = useCastConfigurationStore((state) => state.update);
  const selected = useCastConfigurationStore(selectedCastConfiguration);

  return (
    <Panel
      p="3xl"
      gap="lg"
      title={<PanelHeading order={2}>STATIONS</PanelHeading>}
      action={
        <ActionButton tone="ghost" size="2xs" padding="4px 10px" onClick={() => void add()}>
          + ADD
        </ActionButton>
      }
    >
      {configurations.length === 0 ? (
        <Text size="xs" c={color.textFaint}>
          No stations yet. Add one to say where the mix goes.
        </Text>
      ) : (
        <ScrollArea.Autosize mah={190}>
          {configurations.map((station) => {
            const active = station.id === selectedId;

            return (
              <Group
                key={station.id}
                gap="md"
                wrap="nowrap"
                px="md"
                py="sm"
                onClick={() => !isLive && select(station.id)}
                style={{
                  borderRadius: 'var(--mantine-radius-sm)',
                  border: border(active ? 'acc' : 'line'),
                  background: active ? color.panelHi : undefined,
                  // Changing station while on air would tear the connection
                  // down without being asked to.
                  cursor: isLive ? 'default' : 'pointer',
                  opacity: isLive && !active ? 0.5 : 1,
                  marginBottom: 6,
                }}
              >
                <StatusDot tone={active ? 'accent' : 'inert'} />
                <Box style={{ flex: 1, minWidth: 0 }}>
                  <Text size="sm" truncate>
                    {station.name}
                  </Text>
                  {/* The address is built per protocol: showing the Icecast
                      fields for an Impulse station would print a host it
                      broadcasts nowhere near. */}
                  <Text size="2xs" c={color.textFaintest} mt="3xs" truncate>
                    {castAddress(station)}
                    {station.hasPassword ? '' : ' · no credential'}
                  </Text>
                </Box>
                <DeleteButton
                  onDelete={() => void remove(station.id)}
                  title={`Forget ${station.name}`}
                />
              </Group>
            );
          })}
        </ScrollArea.Autosize>
      )}

      {selected && (
        <>
          <SectionLabel tracking="widest">NAME</SectionLabel>
          <TextInput
            value={selected.name}
            onChange={(event) =>
              void update(selected.id, { ...toInput(selected), name: event.currentTarget.value })
            }
            placeholder="what you call it"
          />
        </>
      )}
    </Panel>
  );
};
