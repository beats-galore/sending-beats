// Channel input controls (device selection, gain, pan)
import { Group, Select, ActionIcon, Stack } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconRefresh } from '@tabler/icons-react';
import { uniqBy } from 'lodash';
import { memo, useCallback, useMemo, useEffect } from 'react';

import { useMixerState } from '../../hooks';
import { AudioSlider } from '../ui';

import type { AudioChannel, AudioDeviceInfo } from '../../types';

const useStyles = createStyles((theme) => ({
  selectFlex: {
    flex: 1,
  },
}));

type ChannelInputsProps = {
  channel: AudioChannel;
  inputDevices: AudioDeviceInfo[];
  onInputDeviceChange: (deviceId: string | null) => void;
  onRefreshDevices: () => void;
};

export const ChannelInputs = memo<ChannelInputsProps>(
  ({ channel, inputDevices, onInputDeviceChange, onRefreshDevices }) => {
    const { classes } = useStyles();
    const { updateChannelGain, updateChannelPan } = useMixerState();

    const handleGainChange = useCallback(
      (gain: number) => {
        updateChannelGain(channel.id, gain);
      },
      [channel.id, updateChannelGain]
    );

    const handlePanChange = useCallback(
      (pan: number) => {
        updateChannelPan(channel.id, pan);
      },
      [channel.id, updateChannelPan]
    );

    const inputDeviceOptions = useMemo(
      () =>
        uniqBy(inputDevices, 'id').map((device) => ({
          value: device.id,
          label: device.name + (device.is_default ? ' (Default)' : ''),
        })),
      [inputDevices]
    );

    // Debug logging to check data (only when data changes)
    useEffect(() => {
      if (inputDevices.length > 0) {
        console.debug('📱 Channel input devices loaded:', {
          count: inputDevices.length,
          firstDevice: inputDevices[0]?.name,
          optionCount: inputDeviceOptions.length,
          inputDeviceOptions,
          allIds: inputDevices.map((d) => d.id),
        });
      }
    }, [inputDevices.length, inputDeviceOptions.length]);

    return (
      <Stack gap="md">
        {/* Input Device Selection */}
        <Group>
          <Select
            placeholder="Select input device..."
            data={inputDeviceOptions}
            value={channel.input_device_id || null}
            onChange={onInputDeviceChange}
            className={classes.selectFlex}
            size="xs"
          />
          <ActionIcon variant="light" onClick={onRefreshDevices} title="Refresh devices" size="sm">
            <IconRefresh size={16} />
          </ActionIcon>
        </Group>

        {/* Gain Control */}
        <AudioSlider
          label="Gain"
          value={channel.gain}
          min={-60}
          max={12}
          step={0.5}
          unit="dB"
          onChange={handleGainChange}
        />

        {/* Pan Control */}
        <AudioSlider
          label="Pan"
          value={channel.pan}
          min={-1}
          max={1}
          step={0.1}
          unit=""
          onChange={handlePanChange}
        />
      </Stack>
    );
  }
);
