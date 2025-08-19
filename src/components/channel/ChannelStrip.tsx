// Professional channel strip component - Compressed horizontal layout
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
  Box,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconChevronDown, IconChevronRight, IconRefresh, IconSettings } from '@tabler/icons-react';
import { memo, useCallback, useMemo, useState } from 'react';

import { useMixerState, useAudioDevices, useChannelLevels } from '../../hooks';

import { ChannelEffects } from './ChannelEffects';
import { ChannelEQ } from './ChannelEQ';
import { ChannelVUMeter } from './ChannelVUMeter';

import type { AudioChannel } from '../../types';

const useStyles = createStyles(() => ({
  channelPaper: {
    width: 260,
    minWidth: 260,
    maxWidth: 260,
    backgroundColor: 'var(--mantine-color-dark-7)',
    borderColor: 'var(--mantine-color-dark-4)',
  },

  vuMeterBox: {
    height: 80,
    display: 'flex',
    justifyContent: 'center',
  },

  sliderContainer: {
    margin: '8px 0',
  },

  effectsBox: {
    backgroundColor: 'var(--mantine-color-dark-8)',
    borderRadius: 4,
  },
}));

// Separate style objects for Mantine components styles prop
const selectStyles = {
  input: {
    fontSize: '10px',
    height: 24,
  },
};

const buttonStyles = {
  root: {
    height: 24,
    fontSize: '10px',
  },
  inner: {
    justifyContent: 'center' as const,
  },
};

type ChannelStripProps = {
  channel: AudioChannel;
};

export const ChannelStrip = memo<ChannelStripProps>(({ channel }) => {
  const { classes } = useStyles();

  const {
    toggleChannelMute,
    toggleChannelSolo,
    setChannelInputDevice,
    updateChannelGain,
    updateChannelPan,
  } = useMixerState();

  const { inputDevices, refreshDevices } = useAudioDevices();
  
  const levels = useChannelLevels(channel.id);

  // State for expandable sections
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showEQ, setShowEQ] = useState(false);
  const [showEffects, setShowEffects] = useState(false);

  const handleMuteToggle = useCallback(() => {
    toggleChannelMute(channel.id);
  }, [channel.id, toggleChannelMute]);

  const handleSoloToggle = useCallback(() => {
    toggleChannelSolo(channel.id);
  }, [channel.id, toggleChannelSolo]);

  const handleInputDeviceChange = useCallback(
    (deviceId: string | null) => {
      if (deviceId) {
        setChannelInputDevice(channel.id, deviceId);
      }
    },
    [channel.id, setChannelInputDevice]
  );

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

  // Convert gain to dB for display
  const gainDb = 20 * Math.log10(Math.max(0.01, channel.gain));

  // Format pan display
  const panDisplay =
    channel.pan === 0
      ? 'CENTER'
      : channel.pan > 0
        ? `R${Math.round(channel.pan * 100)}`
        : `L${Math.round(Math.abs(channel.pan) * 100)}`;

  // Memoize input device options to prevent re-renders
  const inputDeviceOptions = useMemo(() => {
    return inputDevices.map((device) => ({
      value: device.id,
      label: device.name.length > 20 ? `${device.name.substring(0, 20)}...` : device.name,
    }));
  }, [inputDevices]);

  return (
    <Paper p="sm" withBorder radius="md" className={classes.channelPaper}>
      <Stack gap="xs" h="100%">
        {/* Channel Header - Compact */}
        <Group justify="space-between" align="center">
          <Box>
            <Title order={5} size="sm" c="blue" lh={1}>
              CH {channel.id}
            </Title>
            <Text size="xs" c="dimmed" lh={1}>
              {channel.name}
            </Text>
          </Box>

          {/* Advanced Controls Toggle */}
          <ActionIcon
            size="sm"
            variant="subtle"
            onClick={() => setShowAdvanced(!showAdvanced)}
            c={showAdvanced ? 'blue' : 'gray'}
          >
            <IconSettings size={14} />
          </ActionIcon>
        </Group>

        {/* VU Meter - Compact Vertical */}
        <Box className={classes.vuMeterBox}>
          <ChannelVUMeter peakLevel={levels.peak} rmsLevel={levels.rms} />
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
              data={inputDeviceOptions}
              style={{ flex: 1 }}
              styles={selectStyles}
            />
            <ActionIcon size="xs" onClick={refreshDevices} variant="subtle">
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
                { value: 6, label: '+6' },
              ]}
              className={classes.sliderContainer}
            />
          </Box>

          {/* Mute/Solo Buttons - Compact */}
          <Group gap="xs" grow>
            <Button
              size="xs"
              color={channel.muted ? 'red' : 'gray'}
              variant={channel.muted ? 'filled' : 'outline'}
              onClick={handleMuteToggle}
              fullWidth
              styles={buttonStyles}
            >
              {channel.muted ? 'MUTE' : 'MUTE'}
            </Button>

            <Button
              size="xs"
              color={channel.solo ? 'orange' : 'gray'}
              variant={channel.solo ? 'filled' : 'outline'}
              onClick={handleSoloToggle}
              fullWidth
              styles={buttonStyles}
            >
              {channel.solo ? 'SOLO' : 'SOLO'}
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
                  { value: 1, label: 'R' },
                ]}
                className={classes.sliderContainer}
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
              <Box p="xs" className={classes.effectsBox}>
                <ChannelEQ channelId={channel.id} />
              </Box>
            </Collapse>

            {/* Effects Section Toggle */}
            <Button
              size="xs"
              variant="subtle"
              onClick={() => setShowEffects(!showEffects)}
              rightSection={
                showEffects ? <IconChevronDown size={12} /> : <IconChevronRight size={12} />
              }
              justify="flex-start"
              c="blue"
            >
              Effects
            </Button>

            <Collapse in={showEffects}>
              <Box p="xs" className={classes.effectsBox}>
                <ChannelEffects channelId={channel.id} />
              </Box>
            </Collapse>
          </Stack>
        </Collapse>
      </Stack>
    </Paper>
  );
});
