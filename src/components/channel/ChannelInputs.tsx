// Channel input controls (device selection, gain, pan)
import { memo, useCallback } from 'react';
import { Group, Select, ActionIcon, Stack } from '@mantine/core';
import { IconRefresh } from '@tabler/icons-react';
import { AudioChannel, AudioDeviceInfo } from '../../types';
import { AudioSlider } from '../ui';
import { useMixerState } from '../../hooks';

type ChannelInputsProps = {
  channel: AudioChannel;
  inputDevices: AudioDeviceInfo[];
  onInputDeviceChange: (deviceId: string | null) => void;
  onRefreshDevices: () => void;
};

export const ChannelInputs = memo<ChannelInputsProps>(({
  channel,
  inputDevices,
  onInputDeviceChange,
  onRefreshDevices
}) => {
  const { updateChannelGain, updateChannelPan } = useMixerState();

  const handleGainChange = useCallback((gain: number) => {
    updateChannelGain(channel.id, gain);
  }, [channel.id, updateChannelGain]);

  const handlePanChange = useCallback((pan: number) => {
    updateChannelPan(channel.id, pan);
  }, [channel.id, updateChannelPan]);

  const inputDeviceOptions = inputDevices.map(device => ({
    value: device.id,
    label: device.name + (device.is_default ? ' (Default)' : '')
  }));

  return (
    <Stack gap="md">
      {/* Input Device Selection */}
      <Group>
        <Select
          placeholder="Select input device..."
          data={inputDeviceOptions}
          value={channel.input_device_id || null}
          onChange={onInputDeviceChange}
          style={{ flex: 1 }}
          size="xs"
        />
        <ActionIcon 
          variant="light" 
          onClick={onRefreshDevices}
          title="Refresh devices"
          size="sm"
        >
          <IconRefresh size="1rem" />
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
});