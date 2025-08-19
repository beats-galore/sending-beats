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
    width: '100%',
    minHeight: 'fit-content',
    backgroundColor: 'var(--mantine-color-dark-7)',
    borderColor: 'var(--mantine-color-dark-4)',
    overflow: 'hidden',
  },

  mainGroup: {
    width: '100%',
  },

  channelHeader: {
    minWidth: 180,
    flexShrink: 0,
  },

  inputSelect: {
    flex: 1,
    minWidth: 120,
  },

  vuMeterSection: {
    flex: 2,
    minWidth: 200,
  },

  vuMeterContainer: {
    width: '100%',
    height: 20,
    display: 'flex',
    justifyContent: 'center',
    alignItems: 'center',
  },

  sliderSection: {
    width: 200,
    flexShrink: 0,
  },

  sliderBox: {
    width: '100%',
    overflow: 'hidden',
  },

  controlSection: {
    minWidth: 120,
    flexShrink: 0,
  },

  controlButton: {
    width: '100%',
  },

  settingsToggle: {
    flexShrink: 0,
  },

  effectsBox: {
    backgroundColor: 'var(--mantine-color-dark-8)',
    borderRadius: 4,
    marginTop: 8,
  },

  // Custom input styling
  customSelectInput: {
    fontSize: '10px !important',
    height: '24px !important',
  },

  // Custom button styling
  customButton: {
    height: '24px !important',
    fontSize: '10px !important',
    
    '& .mantine-Button-inner': {
      justifyContent: 'center !important',
    },
  },
}));

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
    async (deviceId: string | null) => {
      if (deviceId) {
        try {
          await setChannelInputDevice(channel.id, deviceId);
          console.debug(`✅ Channel ${channel.id} input device set to: ${deviceId}`);
        } catch (error) {
          console.error(`❌ Failed to set input device for channel ${channel.id}:`, error);
        }
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
    <Stack gap={0}>
      <Paper p="sm" withBorder radius="md" className={classes.channelPaper}>
        <Group gap="md" align="flex-start" wrap="nowrap" className={classes.mainGroup}>
          {/* Channel Header with Input Device */}
          <Stack gap={4} className={classes.channelHeader}>
            <Title order={5} size="sm" c="blue" lh={1}>
              CH {channel.id}
            </Title>
            <Box>
              <Group gap={4} align="center">
                <Select
                  size="xs"
                  placeholder="No Input"
                  value={channel.input_device_id || null}
                  onChange={handleInputDeviceChange}
                  data={inputDeviceOptions}
                  className={`${classes.inputSelect} ${classes.customSelectInput}`}
                />
                <ActionIcon size="xs" onClick={refreshDevices} variant="subtle">
                  <IconRefresh size={10} />
                </ActionIcon>
              </Group>
            </Box>
          </Stack>

          {/* VU Meter - Now wider and flexible */}
          <Stack gap={2} className={classes.vuMeterSection}>
            <Text size="xs" c="dimmed" ta="center" lh={1}>
              VU Meter
            </Text>
            <Box className={classes.vuMeterContainer}>
              <ChannelVUMeter peakLevel={levels.peak} rmsLevel={levels.rms} />
            </Box>
          </Stack>

          {/* Gain and Pan Controls - Stacked with more spacing */}
          <Stack gap={24} className={classes.sliderSection}>
            <Box className={classes.sliderBox}>
              <Text size="xs" c="dimmed" ta="center" lh={1} mb={2}>
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
              />
            </Box>

            <Box className={classes.sliderBox}>
              <Text size="xs" c="dimmed" ta="center" lh={1} mb={2}>
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
              />
            </Box>
          </Stack>

          {/* Mute/Solo Buttons */}
          <Stack gap="xs" className={classes.controlSection}>
            <Button
              size="xs"
              color={channel.muted ? 'red' : 'gray'}
              variant={channel.muted ? 'filled' : 'outline'}
              onClick={handleMuteToggle}
              className={`${classes.controlButton} ${classes.customButton}`}
            >
              {channel.muted ? 'MUTE' : 'MUTE'}
            </Button>

            <Button
              size="xs"
              color={channel.solo ? 'orange' : 'gray'}
              variant={channel.solo ? 'filled' : 'outline'}
              onClick={handleSoloToggle}
              className={`${classes.controlButton} ${classes.customButton}`}
            >
              {channel.solo ? 'SOLO' : 'SOLO'}
            </Button>
          </Stack>

          {/* Advanced Controls Toggle */}
          <ActionIcon
            size="sm"
            variant="subtle"
            onClick={() => setShowAdvanced(!showAdvanced)}
            c={showAdvanced ? 'blue' : 'gray'}
            className={classes.settingsToggle}
          >
            <IconSettings size={14} />
          </ActionIcon>
        </Group>
      </Paper>

      {/* Expandable Advanced Controls */}
      <Collapse in={showAdvanced}>
        <Paper p="md" withBorder radius="md" className={classes.effectsBox} mt="xs">
          <Group gap="lg" align="flex-start">
            {/* EQ Section */}
            <Box>
              <Text size="sm" c="blue" mb="xs">
                3-Band EQ
              </Text>
              <ChannelEQ channelId={channel.id} />
            </Box>

            {/* Effects Section */}
            <Box>
              <Text size="sm" c="blue" mb="xs">
                Effects
              </Text>
              <ChannelEffects channelId={channel.id} />
            </Box>
          </Group>
        </Paper>
      </Collapse>
    </Stack>
  );
});
