import { Box, Group, Popover, ScrollArea, Stack, Text, TextInput } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { useCallback, useEffect, useState } from 'react';

import { useConfigurationStore } from '../../../stores/mixer-store';
import { border, color } from '../../../theme/tokens';
import { ActionButton } from '../primitives/ActionButton';
import { SectionLabel } from '../primitives/SectionLabel';
import { StatusDot } from '../primitives/StatusDot';

/** The saved-patch selector in the top bar: what is loaded, and how to change it. */
export const PatchMenu = () => {
  const {
    reusableConfigurations,
    activeSession,
    loadConfigurations,
    selectConfiguration,
    saveSessionAsNewReusable,
  } = useConfigurationStore();

  const [opened, { toggle, close }] = useDisclosure(false);
  const [newName, setNewName] = useState('');

  useEffect(() => {
    void loadConfigurations();
  }, [loadConfigurations]);

  const activeName = activeSession?.configuration.name ?? 'No patch loaded';

  const handleSaveAs = useCallback(() => {
    const name = newName.trim();
    if (!name) {
      return;
    }
    void saveSessionAsNewReusable(name);
    setNewName('');
    close();
  }, [newName, saveSessionAsNewReusable, close]);

  return (
    <Popover opened={opened} onChange={close} position="bottom-start" offset={10} width={330}>
      <Popover.Target>
        <Group
          onClick={toggle}
          gap="sm"
          wrap="nowrap"
          style={{
            padding: '6px 11px',
            borderRadius: 'var(--mantine-radius-sm)',
            cursor: 'pointer',
            background: color.panelNav,
            border: border(opened ? 'acc' : 'line'),
          }}
        >
          <SectionLabel tracking="caps">PATCH</SectionLabel>
          <Text size="xs" fw={600} maw={150} truncate>
            {activeName}
          </Text>
          <Text size="3xs" c={color.textFaint}>
            ▾
          </Text>
        </Group>
      </Popover.Target>

      <Popover.Dropdown>
        <Group
          px="lg"
          py="sm"
          gap="sm"
          style={{ background: color.panelHi, borderBottom: border() }}
        >
          <SectionLabel style={{ flex: 1 }}>SAVED PATCHES</SectionLabel>
          <ActionButton tone="danger" padding="0 2px" size="lg" onClick={close}>
            ×
          </ActionButton>
        </Group>

        <ScrollArea.Autosize mah={250}>
          {reusableConfigurations.length === 0 ? (
            <Text size="xs" c={color.textFaint} ta="center" p="3xl">
              No saved patches yet.
            </Text>
          ) : (
            reusableConfigurations.map(({ configuration, configuredDevices }) => {
              const active =
                configuration.id === activeSession?.configuration.reusableConfigurationId;
              const inputs = configuredDevices.filter((device) => device.isInput).length;
              const outputs = configuredDevices.length - inputs;

              return (
                <Group
                  key={configuration.id}
                  gap="md"
                  wrap="nowrap"
                  px="lg"
                  py="md"
                  onClick={() => {
                    void selectConfiguration(configuration.id);
                    close();
                  }}
                  style={{
                    borderBottom: border(),
                    cursor: 'pointer',
                    background: active ? color.panelHi : undefined,
                  }}
                >
                  <StatusDot tone={active ? 'accent' : 'inert'} />
                  <Box style={{ flex: 1, minWidth: 0 }}>
                    <Text size="sm" truncate>
                      {configuration.name}
                    </Text>
                    <Text size="2xs" c={color.textFaintest} mt="3xs">
                      {inputs} in · {outputs} out
                      {configuration.description ? ` · ${configuration.description}` : ''}
                    </Text>
                  </Box>
                </Group>
              );
            })
          )}
        </ScrollArea.Autosize>

        <Stack gap="sm" p="lg" style={{ borderTop: border() }}>
          <Group gap="sm" wrap="nowrap">
            <TextInput
              value={newName}
              onChange={(event) => setNewName(event.currentTarget.value)}
              placeholder="new patch name"
              style={{ flex: 1, minWidth: 0 }}
            />
            <ActionButton tone="accent" onClick={handleSaveAs}>
              SAVE AS
            </ActionButton>
          </Group>
          <Text size="2xs" c={color.textFaintest} lh="lg">
            Stores every channel, source, gain and destination. Loads back on launch.
          </Text>
        </Stack>
      </Popover.Dropdown>
    </Popover>
  );
};
