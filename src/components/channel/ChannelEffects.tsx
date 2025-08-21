// Channel effects controls - Add from list with collapsible removable tiles
import {
  Stack,
  Group,
  Paper,
  ActionIcon,
  Text,
  Switch,
  Slider,
  Box,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import {
  IconX,
  IconAdjustmentsHorizontal,
  IconVolume,
  IconShield,
} from '@tabler/icons-react';
import { memo, useState, useEffect } from 'react';

import { useChannelEffects } from '../../hooks';
import { audioService } from '../../services';

const useStyles = createStyles((theme) => ({
  effectTile: {
    padding: `${theme.spacing.xs} ${theme.spacing.sm}`,
    backgroundColor: theme.colors.gray[0],
    border: `1px solid ${theme.colors.gray[3]}`,
  },
  effectHeader: {
    cursor: 'pointer',
  },
  compactSlider: {
    width: '180px',
    minWidth: '160px',
  },
  sliderContainer: {
    display: 'flex',
    flexDirection: 'column',
    gap: '4px',
    alignItems: 'center',
  },
  sliderLabel: {
    fontSize: '11px',
    color: theme.colors.blue[4],
    fontWeight: 600,
    textAlign: 'center',
  },
  sliderValue: {
    fontSize: '10px',
    color: theme.colors.gray[0],
    fontFamily: 'monospace',
    textAlign: 'center',
    backgroundColor: theme.colors.dark[8],
    padding: '2px 8px',
    borderRadius: '3px',
    border: `1px solid ${theme.colors.dark[4]}`,
    minWidth: '100px',
  },
}));

const AVAILABLE_EFFECTS = [
  { value: 'eq', label: 'Equalizer', icon: IconAdjustmentsHorizontal },
  { value: 'compressor', label: 'Compressor', icon: IconVolume },
  { value: 'limiter', label: 'Limiter', icon: IconShield },
];

type ChannelEffectsProps = {
  channelId: number;
};

// Compact slider component for effects
const CompactSlider = memo<{
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  unit: string;
  onChange: (value: number) => void;
  disabled?: boolean;
}>(({ label, value, min, max, step, unit, onChange, disabled = false }) => {
  const { classes } = useStyles();

  const displayValue = `${value.toFixed(step < 1 ? 1 : 0)}${unit}`;

  return (
    <Box className={classes.sliderContainer}>
      <Text className={classes.sliderLabel}>{label}</Text>
      <Text className={classes.sliderValue} c="white">
        {displayValue}
      </Text>
      <Slider
        className={classes.compactSlider}
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={onChange}
        disabled={disabled}
        size="sm"
      />
    </Box>
  );
});

export const ChannelEffects = memo<ChannelEffectsProps>(({ channelId }) => {
  const { classes } = useStyles();
  const [activeEffects, setActiveEffects] = useState<string[]>([]);

  const {
    // Compressor
    setCompressorThreshold,
    setCompressorRatio,
    setCompressorAttack,
    setCompressorRelease,
    toggleCompressor,
    getCompressorValues,

    // Limiter
    setLimiterThreshold,
    toggleLimiter,
    getLimiterValues,

    // EQ
    setEQLowGain,
    setEQMidGain,
    setEQHighGain,
    getEQValues,
  } = useChannelEffects(channelId);

  // Load active effects on mount and when effects change
  useEffect(() => {
    const loadActiveEffects = async () => {
      try {
        const effects = await audioService.getChannelEffects(channelId);
        console.log('Loaded effects for channel', channelId, ':', effects);
        setActiveEffects(effects);
      } catch (error) {
        console.error('Failed to load channel effects:', error);
      }
    };
    loadActiveEffects();
  }, [channelId]);

  const handleAddEffect = async (effectType: string) => {
    try {
      await audioService.addChannelEffect(channelId, effectType);
      setActiveEffects((prev) => [...prev, effectType]);
    } catch (error) {
      console.error('Failed to add effect:', error);
    }
  };

  const handleRemoveEffect = async (effectType: string) => {
    try {
      await audioService.removeChannelEffect(channelId, effectType);
      setActiveEffects((prev) => prev.filter((e) => e !== effectType));
    } catch (error) {
      console.error('Failed to remove effect:', error);
    }
  };


  const renderEffectControls = (effectType: string) => {
    switch (effectType) {
      case 'eq': {
        const eq = getEQValues();
        if (!eq) {
          console.log('EQ values are null for channel', channelId);
          return null;
        }
        console.log('EQ values:', eq);
        return (
          <Group gap="md" justify="flex-start">
            <CompactSlider
              label="Low"
              value={eq.low.value}
              min={-12}
              max={12}
              step={0.5}
              unit="dB"
              onChange={setEQLowGain}
            />
            <CompactSlider
              label="Mid"
              value={eq.mid.value}
              min={-12}
              max={12}
              step={0.5}
              unit="dB"
              onChange={setEQMidGain}
            />
            <CompactSlider
              label="High"
              value={eq.high.value}
              min={-12}
              max={12}
              step={0.5}
              unit="dB"
              onChange={setEQHighGain}
            />
          </Group>
        );
      }
      case 'compressor': {
        const compressor = getCompressorValues();
        if (!compressor) return null;
        return (
          <Group gap="md" justify="flex-start" align="flex-start">
            <Box style={{ display: 'flex', flexDirection: 'column', gap: '4px', alignItems: 'center' }}>
              <Text style={{ fontSize: '11px', color: '#339af0', fontWeight: 600 }}>Enable</Text>
              <Switch
                checked={compressor.enabled}
                onChange={toggleCompressor}
                size="sm"
              />
            </Box>
            <CompactSlider
              label="Threshold"
              value={compressor.threshold.value}
              min={-40}
              max={0}
              step={1}
              unit="dB"
              onChange={setCompressorThreshold}
              disabled={!compressor.enabled}
            />
            <CompactSlider
              label="Ratio"
              value={compressor.ratio.value}
              min={1}
              max={10}
              step={0.5}
              unit=":1"
              onChange={setCompressorRatio}
              disabled={!compressor.enabled}
            />
            <CompactSlider
              label="Attack"
              value={compressor.attack.value}
              min={0.1}
              max={100}
              step={0.5}
              unit="ms"
              onChange={setCompressorAttack}
              disabled={!compressor.enabled}
            />
            <CompactSlider
              label="Release"
              value={compressor.release.value}
              min={10}
              max={1000}
              step={10}
              unit="ms"
              onChange={setCompressorRelease}
              disabled={!compressor.enabled}
            />
          </Group>
        );
      }
      case 'limiter': {
        const limiter = getLimiterValues();
        if (!limiter) return null;
        return (
          <Group gap="md" justify="flex-start" align="flex-start">
            <Box style={{ display: 'flex', flexDirection: 'column', gap: '4px', alignItems: 'center' }}>
              <Text style={{ fontSize: '11px', color: '#339af0', fontWeight: 600 }}>Enable</Text>
              <Switch
                checked={limiter.enabled}
                onChange={toggleLimiter}
                size="sm"
              />
            </Box>
            <CompactSlider
              label="Threshold"
              value={limiter.threshold.value}
              min={-12}
              max={0}
              step={0.5}
              unit="dB"
              onChange={setLimiterThreshold}
              disabled={!limiter.enabled}
            />
          </Group>
        );
      }
      default:
        return null;
    }
  };

  return (
    <Stack gap="sm">
      {/* Active Effects - Always Expanded */}
      {activeEffects.map((effectType) => {
        const effectConfig = AVAILABLE_EFFECTS.find((e) => e.value === effectType);
        if (!effectConfig) return null;

        const Icon = effectConfig.icon;

        return (
          <Paper key={effectType} className={classes.effectTile}>
            {/* Effect Header */}
            <Group justify="space-between" mb="xs">
              <Group gap="xs">
                <Icon size={16} />
                <Text size="sm" fw={500}>
                  {effectConfig.label}
                </Text>
              </Group>
              <ActionIcon
                size="xs"
                variant="subtle"
                color="red"
                onClick={() => handleRemoveEffect(effectType)}
              >
                <IconX size={12} />
              </ActionIcon>
            </Group>

            {/* Effect Controls - Always Visible */}
            {renderEffectControls(effectType)}
          </Paper>
        );
      })}

      {activeEffects.length === 0 && (
        <Text size="sm" c="dimmed" ta="center">
          No effects added. Use the + button to add effects.
        </Text>
      )}
    </Stack>
  );
});
