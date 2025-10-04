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
  Box,
  Menu,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import {
  IconRefresh,
  IconPlus,
  IconAdjustmentsHorizontal,
  IconVolume,
  IconShield,
} from '@tabler/icons-react';
import { memo, useCallback, useMemo, useState, useEffect } from 'react';

import { useAudioDevices, useApplicationAudio } from '../../hooks';
import { audioService } from '../../services';
import {
  useAudioEffectsDefaultStore,
  audioEffectsDefaultActions,
} from '../../stores/audio-effects-default-store';
import { useConfigurationStore } from '../../stores/mixer-store';

import type { AudioChannel } from '../../types';
import type { ConfiguredAudioDevice } from '../../types/db';
import type { Identifier } from '../../types/util.types';
import { ChannelEffects } from './ChannelEffects';
import { ChannelVUMeter } from './ChannelVUMeter';

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

const AVAILABLE_EFFECTS = [
  { value: 'eq', label: 'Equalizer', icon: IconAdjustmentsHorizontal },
  { value: 'compressor', label: 'Compressor', icon: IconVolume },
  { value: 'limiter', label: 'Limiter', icon: IconShield },
];

type ChannelStripProps = {
  channel: AudioChannel;
};

export const ChannelStrip = memo<ChannelStripProps>(({ channel }) => {
  const { classes } = useStyles();

  const { inputDevices, refreshDevices } = useAudioDevices();
  const { activeSession, loadConfigurations } = useConfigurationStore();
  const applicationAudio = useApplicationAudio();

  // Select only the effects data we need from the store
  const effectsById = useAudioEffectsDefaultStore((state) => state.effectsById);

  // Extract stable function references
  const { loadEffects, updateGain, updatePan, toggleMute, toggleSolo, getEffectsByDeviceId } =
    audioEffectsDefaultActions;

  const configuredInputDevice = useMemo(() => {
    if (!activeSession?.configuredDevices) {
      console.log(`❌ No activeSession or configuredDevices for channel ${channel.id}`);
      return null;
    }
    const device = activeSession.configuredDevices.find(
      (device) => device.channelNumber === channel.id && device.isInput
    );
    console.log(`🔍 configuredInputDevice for channel ${channel.id}:`, device);
    return device;
  }, [activeSession, channel.id]);

  const deviceEffects = useMemo(() => {
    if (!configuredInputDevice) {
      return null;
    }
    return getEffectsByDeviceId(configuredInputDevice.id);
  }, [configuredInputDevice, effectsById, getEffectsByDeviceId]);

  // State for expandable sections
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showEQ, setShowEQ] = useState(false);
  const [showEffects, setShowEffects] = useState(false);

  const [activeEffects, setActiveEffects] = useState<string[]>([]);

  // Local state for slider values during dragging
  const [localGainDb, setLocalGainDb] = useState<number | null>(null);
  const [localPan, setLocalPan] = useState<number | null>(null);

  useEffect(() => {
    const loadActiveEffects = async () => {
      try {
        const effects = await audioService.getChannelEffects(channel.id);
        setActiveEffects(effects);
      } catch (error) {
        console.error('Failed to load channel effects:', error);
      }
    };
    void loadActiveEffects();
  }, [channel.id]);

  useEffect(() => {
    if (activeSession?.configuration.id) {
      void loadEffects(activeSession.configuration.id);
    }
  }, [activeSession?.configuration.id, loadEffects]);

  const handleMuteToggle = useCallback(() => {
    if (!deviceEffects || !configuredInputDevice || !activeSession) {
      return;
    }
    void toggleMute(deviceEffects.id, configuredInputDevice.id, activeSession.configuration.id);
  }, [deviceEffects, configuredInputDevice, activeSession, toggleMute]);

  const handleSoloToggle = useCallback(() => {
    if (!deviceEffects || !configuredInputDevice || !activeSession) {
      return;
    }
    void toggleSolo(deviceEffects.id, configuredInputDevice.id, activeSession.configuration.id);
  }, [deviceEffects, configuredInputDevice, activeSession, toggleSolo]);

  const handleInputDeviceChange = useCallback(
    async (deviceId: Identifier<ConfiguredAudioDevice> | null) => {
      console.log(
        `🔧 FRONTEND: handleInputDeviceChange called for channel ${channel.id} with deviceId:`,
        deviceId
      );
      if (deviceId) {
        // Check if trying to select an unavailable device
        const isDeviceAvailable = inputDevices.some((device) => device.id === deviceId);
        if (!isDeviceAvailable && !deviceId.startsWith('app-')) {
          console.warn(`⚠️ Attempted to select unavailable device: ${deviceId}`);
          // Could show a toast notification here in the future
          return;
        }

        try {
          // Use the audioService switchInputStream method which handles database sync
          const currentDeviceId = configuredInputDevice?.deviceIdentifier ?? null;
          const isAppAudio = deviceId.startsWith('app-');
          console.log(
            `🔧 FRONTEND: Switching input device: ${currentDeviceId} → ${deviceId}${isAppAudio ? ' (app audio)' : ''}`
          );
          await audioService.switchInputStream(currentDeviceId, deviceId, isAppAudio);
          console.debug(`✅ Channel ${channel.id} input device switched to: ${deviceId}`);

          // **FIX**: Refetch active session to update configured devices in UI
          console.log('🔄 Refetching configurations to update UI...');
          await loadConfigurations();
          console.log('✅ Configurations refetched successfully');
        } catch (error) {
          console.error(`❌ Failed to switch input device for channel ${channel.id}:`, error);
        }
      } else {
        console.log(`🔧 FRONTEND: deviceId is null, not setting input device`);
      }
    },
    [channel.id, configuredInputDevice, inputDevices]
  );

  // Handle gain slider change (during drag)
  const handleGainChange = useCallback((gainDb: number) => {
    setLocalGainDb(gainDb);
  }, []);

  // Handle gain slider change end (on release)
  const handleGainChangeEnd = useCallback(
    (gainDb: number) => {
      console.log('gain change end', gainDb);
      if (!deviceEffects || !configuredInputDevice || !activeSession) {
        return;
      }
      console.log('gain change end actually calling', gainDb);
      // Treat -80dB as complete mute (gain = 0)
      const gain = gainDb <= -79 ? 0 : 10 ** (gainDb / 20);
      void updateGain(
        deviceEffects.id,
        configuredInputDevice.id,
        activeSession.configuration.id,
        gain
      );
      setLocalGainDb(null);
    },
    [deviceEffects, configuredInputDevice, activeSession, updateGain]
  );

  // Handle pan slider change (during drag)
  const handlePanChange = useCallback((pan: number) => {
    setLocalPan(pan);
  }, []);

  // Handle pan slider change end (on release)
  const handlePanChangeEnd = useCallback(
    (pan: number) => {
      if (!deviceEffects || !configuredInputDevice || !activeSession) {
        return;
      }

      void updatePan(
        deviceEffects.id,
        configuredInputDevice.id,
        activeSession.configuration.id,
        pan
      );
      setLocalPan(null);
    },
    [deviceEffects, configuredInputDevice, activeSession, updatePan]
  );

  // Effect handling
  const handleAddEffect = useCallback(
    async (effectType: string) => {
      try {
        console.log('Adding effect', effectType, 'to channel', channel.id);
        await audioService.addChannelEffect(channel.id, effectType);
        setActiveEffects((prev) => [...prev, effectType]);
        setShowAdvanced(true); // Show the effects section
        console.log('Effect added successfully');
      } catch (error) {
        console.error('Failed to add effect:', error);
      }
    },
    [channel.id]
  );

  const availableToAdd = AVAILABLE_EFFECTS.filter(
    (effect) => !activeEffects.includes(effect.value)
  );

  // Use local state during dragging, otherwise use stored value
  const gainDb =
    localGainDb !== null
      ? localGainDb
      : deviceEffects
        ? deviceEffects.gain === 0
          ? -80
          : 20 * Math.log10(Math.max(0.01, deviceEffects.gain))
        : 0;

  const pan = localPan !== null ? localPan : (deviceEffects?.pan ?? 0);

  const panDisplay =
    pan === 0
      ? 'CENTER'
      : pan > 0
        ? `R${Math.round(pan * 100)}`
        : `L${Math.round(Math.abs(pan) * 100)}`;

  // Memoize input device options to prevent re-renders (including application sources)
  const inputDeviceOptions = useMemo(() => {
    const hardwareOptions = inputDevices.map((device) => ({
      value: device.id,
      label: device.name.length > 20 ? `${device.name.substring(0, 20)}...` : device.name,
    }));

    // Add configured device if it's not in the available devices list (missing/unplugged)
    if (configuredInputDevice) {
      const isDeviceAvailable = inputDevices.some(
        (device) => device.id === configuredInputDevice.deviceIdentifier
      );
      if (!isDeviceAvailable) {
        const deviceName =
          configuredInputDevice.deviceName ?? configuredInputDevice.deviceIdentifier;
        hardwareOptions.unshift({
          value: configuredInputDevice.deviceIdentifier,
          label: `${deviceName} (unavailable)`,
        });
      }
    }

    const appOptions = applicationAudio.knownApps.map((app) => ({
      value: `app-${app.pid}`,
      label: app.name.length > 20 ? `${app.name.substring(0, 20)}...` : app.name,
    }));

    console.log('🎛️ ChannelStrip device options:', {
      hardware: hardwareOptions.length,
      applications: appOptions.length,
      totalKnownApps: applicationAudio.knownApps.length,
      configuredInputDevice,
    });

    // Mantine Select expects grouped data in a different format
    if (appOptions.length > 0) {
      return [
        {
          group: 'Hardware Devices',
          items: hardwareOptions,
        },
        {
          group: 'Applications',
          items: appOptions,
        },
      ];
    }
    // No apps available, just return flat array
    return hardwareOptions;
  }, [inputDevices, applicationAudio.knownApps, configuredInputDevice]);

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
                  value={configuredInputDevice?.deviceIdentifier ?? null}
                  onChange={(e) =>
                    void handleInputDeviceChange(e as Identifier<ConfiguredAudioDevice> | null)
                  }
                  data={inputDeviceOptions}
                  className={`${classes.inputSelect} ${classes.customSelectInput}`}
                />
                <ActionIcon size="xs" onClick={() => void refreshDevices()} variant="subtle">
                  <IconRefresh size={10} />
                </ActionIcon>
              </Group>
            </Box>
          </Stack>

          {/* VU Meter - Now wider and flexible */}
          <Stack gap={2} className={classes.vuMeterSection}>
            <Box className={classes.vuMeterContainer}>
              <ChannelVUMeter channelId={channel.id} />
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
                min={-50}
                max={20}
                step={0.5}
                value={gainDb}
                onChange={handleGainChange}
                onChangeEnd={handleGainChangeEnd}
                marks={[
                  { value: -50, label: '-50' },
                  { value: 0, label: '0' },
                  { value: 20, label: '+20' },
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
                value={pan}
                onChange={handlePanChange}
                onChangeEnd={handlePanChangeEnd}
                marks={[
                  { value: -1, label: 'L' },
                  { value: 0, label: 'C' },
                  { value: 1, label: 'R' },
                ]}
              />
            </Box>
          </Stack>

          <Stack gap="xs" className={classes.controlSection}>
            <Button
              size="xs"
              color={deviceEffects?.muted ? 'red' : 'gray'}
              variant={deviceEffects?.muted ? 'filled' : 'outline'}
              onClick={handleMuteToggle}
              className={`${classes.controlButton} ${classes.customButton}`}
            >
              MUTE
            </Button>

            <Button
              size="xs"
              color={deviceEffects?.solo ? 'orange' : 'gray'}
              variant={deviceEffects?.solo ? 'filled' : 'outline'}
              onClick={handleSoloToggle}
              className={`${classes.controlButton} ${classes.customButton}`}
            >
              SOLO
            </Button>
          </Stack>

          {/* Add Effects Menu */}
          {availableToAdd.length > 0 ? (
            <Menu position="bottom-end" withArrow>
              <Menu.Target>
                <ActionIcon
                  size="sm"
                  variant="outline"
                  color="blue"
                  className={classes.settingsToggle}
                >
                  <IconPlus size={14} />
                </ActionIcon>
              </Menu.Target>
              <Menu.Dropdown>
                {availableToAdd.map((effect) => (
                  <Menu.Item
                    key={effect.value}
                    leftSection={<effect.icon size={16} />}
                    onClick={() => void handleAddEffect(effect.value)}
                  >
                    {effect.label}
                  </Menu.Item>
                ))}
              </Menu.Dropdown>
            </Menu>
          ) : (
            <ActionIcon
              size="sm"
              variant="subtle"
              onClick={() => setShowAdvanced(!showAdvanced)}
              c={showAdvanced ? 'blue' : 'gray'}
              className={classes.settingsToggle}
            >
              <IconPlus size={14} />
            </ActionIcon>
          )}
        </Group>
      </Paper>

      {/* Expandable Advanced Controls */}
      <Collapse in={showAdvanced}>
        <Paper p="md" withBorder radius="md" className={classes.effectsBox} mt="xs">
          <ChannelEffects channelId={channel.id} key={`${channel.id}-${activeEffects.join(',')}`} />
        </Paper>
      </Collapse>
    </Stack>
  );
});
