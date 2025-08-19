// Professional channel strip component - Compressed horizontal layout
import { memo, useCallback, useMemo, useState } from 'react';
import { 
  Paper, 
  Group, 
  Stack, 
  Title, 
  Text,
  Button,
  Select,
  Slider,
  Collapse,
  ActionIcon,
  Divider,
  Box
} from '@mantine/core';
import { 
  IconChevronDown,
  IconChevronRight,
  IconRefresh,
  IconSettings
} from '@tabler/icons-react';
import { AudioChannel, AudioDeviceInfo } from '../../types';
import { useMixerState, useVUMeterData } from '../../hooks';
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
    setChannelInputDevice,
    updateChannelGain,
    updateChannelPan
  } = useMixerState();

  const { getChannelLevels } = useVUMeterData();
  
  // State for expandable sections
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showEQ, setShowEQ] = useState(false);
  const [showEffects, setShowEffects] = useState(false);
  
  // Memoize levels to prevent infinite re-renders from new object creation
  const levels = useMemo(() => {
    return getChannelLevels(channel.id);
  }, [getChannelLevels, channel.id, channel.peak_level, channel.rms_level]);

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

  const handleGainChange = useCallback((gain: number) => {
    updateChannelGain(channel.id, gain);
  }, [channel.id, updateChannelGain]);

  const handlePanChange = useCallback((pan: number) => {
    updateChannelPan(channel.id, pan);
  }, [channel.id, updateChannelPan]);

  // Convert gain to dB for display
  const gainDb = 20 * Math.log10(Math.max(0.01, channel.gain));
  
  // Format pan display
  const panDisplay = channel.pan === 0 ? "CENTER" :
    channel.pan > 0 ? `R${Math.round(channel.pan * 100)}` :
    `L${Math.round(Math.abs(channel.pan) * 100)}`;

  return (
    <Paper 
      p="sm" 
      withBorder 
      radius="md" 
      style={{ 
        width: 260, 
        minWidth: 260, 
        maxWidth: 260,
        backgroundColor: 'var(--mantine-color-dark-7)',
        borderColor: 'var(--mantine-color-dark-4)'
      }}
    >
      <Stack gap="xs" h="100%">
        {/* Channel Header - Compact */}
        <Group justify="space-between" align="center">
          <Box>
            <Title order={5} size="sm" c="blue" lh={1}>CH {channel.id}</Title>
            <Text size="xs" c="dimmed" lh={1}>{channel.name}</Text>
          </Box>
          
          {/* Advanced Controls Toggle */}
          <ActionIcon 
            size="sm" 
            variant="subtle"
            onClick={() => setShowAdvanced(!showAdvanced)}
            c={showAdvanced ? "blue" : "gray"}
          >
            <IconSettings size={14} />
          </ActionIcon>
        </Group>

        {/* VU Meter - Compact Vertical */}
        <Box style={{ height: 80, display: 'flex', justifyContent: 'center' }}>
          <ChannelVUMeter
            peakLevel={levels.peak}
            rmsLevel={levels.rms}
          />
        </Box>

        {/* Essential Controls - Always Visible */}
        <Stack gap="xs">
          {/* Input Device Selection - Compact */}
          <Group gap={4} align="center">
            <Select
              size="xs"
              placeholder="No Input"
              value={channel.input_device_id || null}
              onChange={handleInputDeviceChange}
              data={inputDevices.map(device => ({
                value: device.id,
                label: device.name.length > 20 ? device.name.substring(0, 20) + "..." : device.name
              }))}
              style={{ flex: 1 }}
              styles={{
                input: { fontSize: '10px', height: 24 }
              }}
            />
            <ActionIcon size="xs" onClick={onRefreshDevices} variant="subtle">
              <IconRefresh size={10} />
            </ActionIcon>
          </Group>

          {/* Gain Control - Horizontal Slider */}
          <Box>
            <Text size="xs" c="dimmed" ta="center" lh={1}>
              Gain: {gainDb.toFixed(1)}dB
            </Text>
            <Slider
              size="xs"
              min={-20}
              max={6}
              step={0.5}
              value={gainDb}
              onChange={(value) => handleGainChange(Math.pow(10, value / 20))}
              marks={[
                { value: -20, label: '-20' },
                { value: 0, label: '0' },
                { value: 6, label: '+6' }
              ]}
              style={{ margin: '8px 0' }}
            />
          </Box>

          {/* Mute/Solo Buttons - Compact */}
          <Group gap="xs" grow>
            <Button
              size="xs"
              color={channel.muted ? "red" : "gray"}
              variant={channel.muted ? "filled" : "outline"}
              onClick={handleMuteToggle}
              fullWidth
              styles={{
                root: { height: 24, fontSize: '10px' },
                inner: { justifyContent: 'center' }
              }}
            >
              {channel.muted ? "MUTE" : "MUTE"}
            </Button>

            <Button
              size="xs"
              color={channel.solo ? "orange" : "gray"}
              variant={channel.solo ? "filled" : "outline"}
              onClick={handleSoloToggle}
              fullWidth
              styles={{
                root: { height: 24, fontSize: '10px' },
                inner: { justifyContent: 'center' }
              }}
            >
              {channel.solo ? "SOLO" : "SOLO"}
            </Button>
          </Group>
        </Stack>

        {/* Expandable Advanced Controls */}
        <Collapse in={showAdvanced}>
          <Stack gap="xs" mt="xs">
            <Divider size="xs" />
            
            {/* Pan Control */}
            <Box>
              <Text size="xs" c="dimmed" ta="center" lh={1}>
                Pan: {panDisplay}
              </Text>
              <Slider
                size="xs"
                min={-1}
                max={1}
                step={0.05}
                value={channel.pan}
                onChange={handlePanChange}
                marks={[
                  { value: -1, label: 'L' },
                  { value: 0, label: 'C' },
                  { value: 1, label: 'R' }
                ]}
                style={{ margin: '8px 0' }}
              />
            </Box>

            {/* EQ Section Toggle */}
            <Button
              size="xs"
              variant="subtle"
              onClick={() => setShowEQ(!showEQ)}
              rightSection={showEQ ? <IconChevronDown size={12} /> : <IconChevronRight size={12} />}
              justify="flex-start"
              c="blue"
            >
              3-Band EQ
            </Button>
            
            <Collapse in={showEQ}>
              <Box p="xs" style={{ backgroundColor: 'var(--mantine-color-dark-8)', borderRadius: 4 }}>
                <ChannelEQ channelId={channel.id} />
              </Box>
            </Collapse>

            {/* Effects Section Toggle */}
            <Button
              size="xs"
              variant="subtle"
              onClick={() => setShowEffects(!showEffects)}
              rightSection={showEffects ? <IconChevronDown size={12} /> : <IconChevronRight size={12} />}
              justify="flex-start"
              c="blue"
            >
              Effects
            </Button>
            
            <Collapse in={showEffects}>
              <Box p="xs" style={{ backgroundColor: 'var(--mantine-color-dark-8)', borderRadius: 4 }}>
                <ChannelEffects channelId={channel.id} />
              </Box>
            </Collapse>
          </Stack>
        </Collapse>
      </Stack>
    </Paper>
  );
});