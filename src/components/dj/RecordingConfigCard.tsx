import {
  Card,
  Stack,
  Group,
  Title,
  TextInput,
  Select,
  Switch,
  NumberInput,
  Textarea,
  Button,
  Badge,
  Text,
  Modal,
  ScrollArea,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import {
  IconSettings,
  IconFolder,
  IconMusic,
  IconDeviceFloppy,
  IconTrash,
  IconPlus,
} from '@tabler/icons-react';
import { memo, useState, useCallback, useEffect } from 'react';

import { useRecording } from '../../hooks/use-recording';

import { MetadataForm } from './MetadataForm';

import type {
  RecordingConfig,
  RecordingFormat,
  RecordingMetadata,
  MetadataPreset,
} from '../../types/audio.types';

type RecordingConfigCardProps = {
  disabled?: boolean;
};

const useStyles = createStyles((theme) => ({
  configCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },

  configItem: {
    backgroundColor: theme.colors.dark[7],
    padding: theme.spacing.sm,
    borderRadius: theme.radius.md,
    cursor: 'pointer',
    transition: 'background-color 0.2s ease',

    '&:hover': {
      backgroundColor: theme.colors.dark[5],
    },
  },

  activeConfig: {
    borderLeft: `3px solid ${theme.colors.blue[5]}`,
  },

  modalContent: {
    maxHeight: '80vh',
  },

  sectionHeader: {
    borderBottom: `1px solid ${theme.colors.dark[4]}`,
    paddingBottom: theme.spacing.xs,
    marginBottom: theme.spacing.sm,
  },
}));

const formatOptions = [
  { value: 'wav', label: 'WAV (Uncompressed)', description: 'Best quality, large files' },
  { value: 'mp3_320', label: 'MP3 320kbps', description: 'High quality, reasonable size' },
  { value: 'mp3_256', label: 'MP3 256kbps', description: 'Good quality, balanced' },
  { value: 'mp3_192', label: 'MP3 192kbps', description: 'Standard quality' },
  { value: 'mp3_128', label: 'MP3 128kbps', description: 'Compact size' },
  { value: 'flac', label: 'FLAC (Lossless)', description: 'Lossless compression' },
];

const templateVariables = [
  { variable: '{timestamp}', description: 'Unix timestamp' },
  { variable: '{title}', description: 'Track title' },
  { variable: '{artist}', description: 'Artist name' },
  { variable: '{album}', description: 'Album name' },
  { variable: '{genre}', description: 'Music genre' },
];

const parseFormatValue = (value: string): RecordingFormat => {
  switch (value) {
    case 'wav':
      return { wav: {} };
    case 'mp3_320':
      return { mp3: { bitrate: 320 } };
    case 'mp3_256':
      return { mp3: { bitrate: 256 } };
    case 'mp3_192':
      return { mp3: { bitrate: 192 } };
    case 'mp3_128':
      return { mp3: { bitrate: 128 } };
    case 'flac':
      return { flac: { compression_level: 5 } };
    default:
      return { mp3: { bitrate: 192 } };
  }
};

const formatToValue = (format: RecordingFormat): string => {
  if (format.wav) return 'wav';
  if (format.mp3) {
    if (format.mp3.bitrate === 320) return 'mp3_320';
    if (format.mp3.bitrate === 256) return 'mp3_256';
    if (format.mp3.bitrate === 128) return 'mp3_128';
    return 'mp3_192';
  }
  if (format.flac) return 'flac';
  return 'mp3_192';
};

export const RecordingConfigCard = memo<RecordingConfigCardProps>(({ disabled = false }) => {
  const { classes } = useStyles();
  const { configs, actions } = useRecording();

  const [editingConfig, setEditingConfig] = useState<RecordingConfig | null>(null);
  const [modalOpened, setModalOpened] = useState(false);
  const [selectedConfigId, setSelectedConfigId] = useState<string>('');
  const [metadataPresets, setMetadataPresets] = useState<MetadataPreset[]>([]);
  const [recordingPresets, setRecordingPresets] = useState<RecordingConfig[]>([]);

  // Initialize selected config
  useEffect(() => {
    if (configs.length > 0 && !selectedConfigId) {
      setSelectedConfigId(configs[0].id);
    }
  }, [configs, selectedConfigId]);

  // Load presets on mount
  useEffect(() => {
    const loadPresets = async () => {
      console.log('reloading presets?');
      try {
        const [metaPresets, recPresets] = await Promise.all([
          actions.getMetadataPresets(),
          actions.getRecordingPresets(),
        ]);
        setMetadataPresets(metaPresets);
        setRecordingPresets(recPresets);
      } catch (err) {
        console.error('Failed to load presets:', err);
      }
    };
    void loadPresets();
  }, [actions]);

  const selectedConfig = configs.find((c) => c.id === selectedConfigId);

  const handleCreateNew = useCallback(async () => {
    try {
      const defaultConfig = await actions.createDefaultConfig();
      setEditingConfig({
        ...defaultConfig,
        name: 'New Configuration',
      });
      setModalOpened(true);
    } catch (err) {
      console.error('Failed to create default config:', err);
    }
  }, [actions]);

  const handleCreateFromPreset = useCallback((preset: RecordingConfig) => {
    setEditingConfig({
      ...preset,
      name: `${preset.name} (Copy)`,
      id: crypto.randomUUID(), // Generate new ID
    });
    setModalOpened(true);
  }, []);

  const handleEdit = useCallback((config: RecordingConfig) => {
    setEditingConfig({ ...config });
    setModalOpened(true);
  }, []);

  const handleSave = useCallback(async () => {
    if (!editingConfig) return;

    try {
      await actions.saveConfig(editingConfig);
      setModalOpened(false);
      setEditingConfig(null);
    } catch (err) {
      console.error('Failed to save config:', err);
    }
  }, [editingConfig, actions]);

  const handleCancel = useCallback(() => {
    setModalOpened(false);
    setEditingConfig(null);
  }, []);

  const updateEditingConfig = useCallback(
    (updates: Partial<RecordingConfig>) => {
      if (editingConfig) {
        setEditingConfig({ ...editingConfig, ...updates });
      }
    },
    [editingConfig]
  );

  return (
    <>
      <Card className={classes.configCard} padding="lg" withBorder>
        <Stack gap="md">
          <Group justify="space-between" align="center">
            <Group gap="xs">
              <IconSettings size={20} color="#339af0" />
              <Title order={4} c="blue.4">
                Recording Presets
              </Title>
            </Group>

            <Group gap="xs">
              <Button
                leftSection={<IconPlus size={16} />}
                onClick={handleCreateNew}
                size="sm"
                variant="light"
                disabled={disabled}
              >
                New Preset
              </Button>

              {/* Recording Preset Dropdown */}
              {recordingPresets.length > 0 && (
                <Select
                  placeholder="Or use template"
                  size="sm"
                  data={recordingPresets.map((p) => ({ value: p.id, label: p.name }))}
                  onChange={(value) => {
                    const preset = recordingPresets.find((p) => p.id === value);
                    if (preset) handleCreateFromPreset(preset);
                  }}
                  style={{ minWidth: 150 }}
                />
              )}
            </Group>
          </Group>

          {/* Config List */}
          <Stack gap="xs">
            {configs.length === 0 ? (
              <Text c="dimmed" size="sm" ta="center">
                No presets saved. Create your first preset above.
              </Text>
            ) : (
              configs.map((config) => (
                <div
                  key={config.id}
                  className={`${classes.configItem} ${config.id === selectedConfigId ? classes.activeConfig : ''}`}
                  onClick={() => setSelectedConfigId(config.id)}
                >
                  <Group justify="space-between" align="center">
                    <Stack gap={2}>
                      <Text size="sm" fw={500}>
                        {config.name}
                      </Text>
                      <Group gap="xs">
                        <Badge size="xs" variant="light">
                          {formatToValue(config.format).toUpperCase()}
                        </Badge>
                        {config.auto_stop_on_silence && (
                          <Badge size="xs" variant="outline" color="yellow">
                            Auto-stop
                          </Badge>
                        )}
                        {config.max_duration_minutes && (
                          <Badge size="xs" variant="outline" color="orange">
                            Max {config.max_duration_minutes}m
                          </Badge>
                        )}
                      </Group>
                    </Stack>

                    <Group gap="xs">
                      <Button
                        size="xs"
                        variant="subtle"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleEdit(config);
                        }}
                      >
                        Edit
                      </Button>
                    </Group>
                  </Group>
                </div>
              ))
            )}
          </Stack>

          {/* Selected Config Details */}
          {selectedConfig && (
            <Stack gap="sm">
              <Text size="sm" fw={500} className={classes.sectionHeader}>
                Current Preset: {selectedConfig.name}
              </Text>

              <Group>
                <Text size="xs" c="dimmed">
                  Format:
                </Text>
                <Text size="xs">{formatToValue(selectedConfig.format).toUpperCase()}</Text>
              </Group>

              <Group>
                <Text size="xs" c="dimmed">
                  Template:
                </Text>
                <Text size="xs" style={{ fontFamily: 'monospace' }}>
                  {selectedConfig.filename_template}
                </Text>
              </Group>

              {selectedConfig.metadata.title && (
                <Group>
                  <Text size="xs" c="dimmed">
                    Title:
                  </Text>
                  <Text size="xs">{selectedConfig.metadata.title}</Text>
                </Group>
              )}

              {selectedConfig.metadata.artist && (
                <Group>
                  <Text size="xs" c="dimmed">
                    Artist:
                  </Text>
                  <Text size="xs">{selectedConfig.metadata.artist}</Text>
                </Group>
              )}
            </Stack>
          )}
        </Stack>
      </Card>

      {/* Config Editor Modal */}
      <Modal
        opened={modalOpened}
        onClose={handleCancel}
        title="Recording Configuration"
        size="lg"
        scrollAreaComponent={ScrollArea.Autosize}
      >
        {editingConfig && (
          <Stack gap="md" className={classes.modalContent}>
            {/* Basic Settings */}
            <Stack gap="sm">
              <Text fw={500} className={classes.sectionHeader}>
                Basic Settings
              </Text>

              <TextInput
                label="Configuration Name"
                value={editingConfig.name}
                onChange={(e) => updateEditingConfig({ name: e.target.value })}
                required
              />

              <Select
                label="Recording Format"
                description="Choose quality vs file size trade-off"
                value={formatToValue(editingConfig.format)}
                onChange={(value) =>
                  value && updateEditingConfig({ format: parseFormatValue(value) })
                }
                data={formatOptions}
              />

              <TextInput
                label="Output Directory"
                description="Leave empty for default music directory"
                placeholder="/path/to/recordings"
                value={editingConfig.output_directory}
                onChange={(e) => updateEditingConfig({ output_directory: e.target.value })}
                rightSection={<IconFolder size={16} />}
              />

              <TextInput
                label="Filename Template"
                description="Use variables like {timestamp}, {title}, {artist}"
                value={editingConfig.filename_template}
                onChange={(e) => updateEditingConfig({ filename_template: e.target.value })}
                style={{ fontFamily: 'monospace' }}
              />

              <Text size="xs" c="dimmed">
                Template variables: {templateVariables.map((v) => v.variable).join(', ')}
              </Text>
            </Stack>

            {/* Comprehensive Metadata Form */}
            <Stack gap="sm">
              <Text fw={500} className={classes.sectionHeader}>
                Recording Metadata
              </Text>

              <MetadataForm
                metadata={editingConfig.metadata}
                onChange={(metadata) => updateEditingConfig({ metadata })}
                presets={metadataPresets}
                showPresetButtons
              />
            </Stack>

            {/* Advanced Settings */}
            <Stack gap="sm">
              <Text fw={500} className={classes.sectionHeader}>
                Advanced Settings
              </Text>

              <Switch
                label="Auto-stop on silence"
                description="Automatically stop recording after silence period"
                checked={editingConfig.auto_stop_on_silence}
                onChange={(e) =>
                  updateEditingConfig({ auto_stop_on_silence: e.currentTarget.checked })
                }
              />

              {editingConfig.auto_stop_on_silence && (
                <Group grow>
                  <NumberInput
                    label="Silence Threshold (dB)"
                    value={editingConfig.silence_threshold_db}
                    onChange={(value) =>
                      typeof value === 'number' &&
                      updateEditingConfig({ silence_threshold_db: value })
                    }
                    min={-100}
                    max={0}
                    step={5}
                  />
                  <NumberInput
                    label="Silence Duration (seconds)"
                    value={editingConfig.silence_duration_sec}
                    onChange={(value) =>
                      typeof value === 'number' &&
                      updateEditingConfig({ silence_duration_sec: value })
                    }
                    min={1}
                    max={60}
                  />
                </Group>
              )}

              <Group grow>
                <NumberInput
                  label="Max Duration (minutes)"
                  description="0 = no limit"
                  value={editingConfig.max_duration_minutes || 0}
                  onChange={(value) =>
                    updateEditingConfig({
                      max_duration_minutes:
                        typeof value === 'number' && value > 0 ? value : undefined,
                    })
                  }
                  min={0}
                  max={1440}
                />
                <NumberInput
                  label="Max File Size (MB)"
                  description="0 = no limit"
                  value={editingConfig.max_file_size_mb || 0}
                  onChange={(value) =>
                    updateEditingConfig({
                      max_file_size_mb: typeof value === 'number' && value > 0 ? value : undefined,
                    })
                  }
                  min={0}
                  max={10000}
                />
              </Group>

              <Group grow>
                <NumberInput
                  label="Sample Rate (Hz)"
                  value={editingConfig.sample_rate}
                  onChange={(value) =>
                    typeof value === 'number' && updateEditingConfig({ sample_rate: value })
                  }
                  min={8000}
                  max={192000}
                  step={1000}
                />
                <NumberInput
                  label="Channels"
                  value={editingConfig.channels}
                  onChange={(value) =>
                    typeof value === 'number' && updateEditingConfig({ channels: value })
                  }
                  min={1}
                  max={8}
                />
              </Group>
            </Stack>

            {/* Actions */}
            <Group justify="flex-end" gap="sm" pt="md">
              <Button variant="subtle" onClick={handleCancel}>
                Cancel
              </Button>
              <Button
                leftSection={<IconDeviceFloppy size={16} />}
                onClick={handleSave}
                disabled={!editingConfig.name.trim()}
              >
                Save Configuration
              </Button>
            </Group>
          </Stack>
        )}
      </Modal>
    </>
  );
});

RecordingConfigCard.displayName = 'RecordingConfigCard';
