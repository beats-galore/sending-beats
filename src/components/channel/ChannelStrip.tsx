// Professional channel strip component with full controls
import { memo, useCallback } from 'react';
import { 
  Paper, 
  Group, 
  Stack, 
  Title, 
  Text,
  Button
} from '@mantine/core';
import { IconVolume, IconVolumeOff } from '@tabler/icons-react';
import { AudioChannel, AudioDeviceInfo } from '../../types';
import { useMixerState, useVUMeterData } from '../../hooks';
import { ChannelInputs } from './ChannelInputs';
import { ChannelEQ } from './ChannelEQ';
import { ChannelEffects } from './ChannelEffects';
import { ChannelVUMeter } from './ChannelVUMeter';

type ChannelStripProps = {
  channel: AudioChannel;
  inputDevices: AudioDeviceInfo[];
  onRefreshDevices: () => void;
};

export const ChannelStrip = memo<ChannelStripProps>(({
  channel,
  inputDevices,
  onRefreshDevices
}) => {
  const { 
    toggleChannelMute,
    toggleChannelSolo,
    setChannelInputDevice
  } = useMixerState();

  const { getChannelLevels } = useVUMeterData();
  const levels = getChannelLevels(channel.id);

  const handleMuteToggle = useCallback(() => {
    toggleChannelMute(channel.id);
  }, [channel.id, toggleChannelMute]);

  const handleSoloToggle = useCallback(() => {
    toggleChannelSolo(channel.id);
  }, [channel.id, toggleChannelSolo]);

  const handleInputDeviceChange = useCallback((deviceId: string | null) => {
    if (deviceId) {
      setChannelInputDevice(channel.id, deviceId);
    }
  }, [channel.id, setChannelInputDevice]);

  return (
    <Paper p="md" withBorder radius="md">
      <Stack gap="md">
        {/* Channel Header - Horizontal Layout */}
        <Group justify="space-between">
          <div>
            <Title order={4}>CH {channel.id}</Title>
            <Text size="sm" c="dimmed">{channel.name}</Text>
          </div>

          {/* Channel VU Meter */}
          <ChannelVUMeter
            peakLevel={levels.peak}
            rmsLevel={levels.rms}
          />

          {/* Mute/Solo Controls */}
          <Group gap="xs">
            <Button
              size="xs"
              color={channel.muted ? "red" : "gray"}
              variant={channel.muted ? "filled" : "outline"}
              onClick={handleMuteToggle}
              leftSection={channel.muted ? <IconVolumeOff size="0.8rem" /> : <IconVolume size="0.8rem" />}
            >
              {channel.muted ? "MUTED" : "MUTE"}
            </Button>

            <Button
              size="xs"
              color={channel.solo ? "orange" : "gray"}
              variant={channel.solo ? "filled" : "outline"}
              onClick={handleSoloToggle}
            >
              {channel.solo ? "SOLO" : "SOLO"}
            </Button>
          </Group>
        </Group>

        {/* Main Channel Controls Grid */}
        <Group grow align="flex-start">
          {/* Input Controls */}
          <Stack>
            <Title order={6} c="blue">Input</Title>
            <ChannelInputs
              channel={channel}
              inputDevices={inputDevices}
              onInputDeviceChange={handleInputDeviceChange}
              onRefreshDevices={onRefreshDevices}
            />
          </Stack>

          {/* EQ Controls */}
          <Stack>
            <Title order={6} c="blue">EQ</Title>
            <ChannelEQ channelId={channel.id} />
          </Stack>

          {/* Effects Controls */}
          <Stack>
            <Title order={6} c="blue">Effects</Title>
            <ChannelEffects channelId={channel.id} />
          </Stack>
        </Group>
      </Stack>
    </Paper>
  );
});