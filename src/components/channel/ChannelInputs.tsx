// Channel input controls (device selection, gain, pan)
import { Stack } from '@mantine/core';
import { memo, useCallback } from 'react';

import { useMixerState } from '../../hooks';
import { AudioSlider, EnhancedDeviceSelector } from '../ui';

import type { AudioChannel, AudioDeviceInfo } from '../../types';

type ChannelInputsProps = {
  channel: AudioChannel;
  inputDevices: AudioDeviceInfo[];
  onInputDeviceChange: (deviceId: string | null) => void;
  onRefreshDevices: () => void;
};

export const ChannelInputs = memo<ChannelInputsProps>(
  ({ channel, inputDevices, onInputDeviceChange, onRefreshDevices }) => {
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

    return (
      <Stack gap="md">
        {/* Enhanced Input Device Selection with Application Sources */}
        <EnhancedDeviceSelector
          inputDevices={inputDevices}
          selectedDeviceId={channel.input_device_id}
          onInputDeviceChange={onInputDeviceChange}
          onRefreshDevices={onRefreshDevices}
        />

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
