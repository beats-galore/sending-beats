import { Box, Group, ScrollArea, Text } from '@mantine/core';
import { useDisclosure, useHover } from '@mantine/hooks';

import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import type { AudioDeviceInfo } from '../../../types';
import { ActionButton } from '../primitives/ActionButton';
import { DashedTarget } from '../primitives/DashedTarget';
import { SectionLabel } from '../primitives/SectionLabel';
import { StatusDot } from '../primitives/StatusDot';

const { destination } = layout;

type AddDestinationProps = {
  top: number;
  available: AudioDeviceInfo[];
  /** Stations not yet on this patch, which can be broadcast to like a device */
  castOptions: { id: string; name: string }[];
  onPick: (deviceId: string) => void;
  onPickCast: (castConfigurationId: string) => void;
};

type DeviceRowProps = {
  device: AudioDeviceInfo;
  onPick: (deviceId: string) => void;
};

const DeviceRow = ({ device, onPick }: DeviceRowProps) => {
  const { hovered, ref } = useHover();

  return (
    <Group
      ref={ref}
      onClick={() => onPick(device.id)}
      gap="sm"
      wrap="nowrap"
      px="md"
      py="sm"
      style={{
        borderRadius: 'var(--mantine-radius-sm)',
        cursor: 'pointer',
        background: hovered ? color.panelHi : undefined,
      }}
    >
      <StatusDot tone="inert" />
      <Text size="sm" truncate style={{ flex: 1 }}>
        {device.name}
      </Text>
      <Text size="2xs" c={color.textFaintest} style={{ flex: 'none' }}>
        {device.supported_channels[0] ?? 2} ch
      </Text>
    </Group>
  );
};

/** Adds another output device to the patch. */
export const AddDestination = ({
  top,
  available,
  castOptions,
  onPick,
  onPickCast,
}: AddDestinationProps) => {
  const [picking, { open, close }] = useDisclosure(false);

  if (!picking) {
    return (
      <Box style={{ position: 'absolute', left: destination.x, top, width: destination.width }}>
        <DashedTarget label="+ ADD A DESTINATION" height={destination.addHeight} onClick={open} />
      </Box>
    );
  }

  return (
    <Box
      style={{
        position: 'absolute',
        left: destination.x,
        top,
        width: destination.width,
        background: color.panel,
        border: border('acc'),
        borderRadius: 'var(--mantine-radius-xl)',
        boxShadow: 'var(--mantine-shadow-sm)',
        overflow: 'hidden',
      }}
    >
      <Group h={30} px="lg" gap="sm" style={{ background: color.panelHi, borderBottom: border() }}>
        <SectionLabel style={{ flex: 1 }}>ADD A DESTINATION</SectionLabel>
        <ActionButton tone="danger" padding="0 2px" size="lg" onClick={close}>
          ×
        </ActionButton>
      </Group>

      <ScrollArea.Autosize mah={210} p="sm">
        {/* Stations first: a broadcast is the destination most patches are
            built around, and it is the one that is not a piece of hardware. */}
        {castOptions.map((station) => (
          <Group
            key={station.id}
            onClick={() => onPickCast(station.id)}
            gap="sm"
            wrap="nowrap"
            px="md"
            py="sm"
            style={{ borderRadius: 'var(--mantine-radius-sm)', cursor: 'pointer' }}
          >
            <StatusDot tone="inert" />
            <Text size="sm" truncate style={{ flex: 1 }}>
              {station.name}
            </Text>
            <Text size="2xs" c={color.textFaintest} style={{ flex: 'none' }}>
              broadcast
            </Text>
          </Group>
        ))}

        {available.length === 0 && castOptions.length === 0 ? (
          <Text size="xs" c={color.textFaint} ta="center" p="xl">
            Every output and station is already patched.
          </Text>
        ) : (
          available.map((device) => (
            <DeviceRow
              key={device.id}
              device={device}
              onPick={(deviceId) => {
                onPick(deviceId);
                close();
              }}
            />
          ))
        )}
      </ScrollArea.Autosize>
    </Box>
  );
};
