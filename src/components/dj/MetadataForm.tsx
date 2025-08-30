import { Stack, Group, Title, TextInput, NumberInput, Textarea, Button, Select, Divider, Text, Grid, ActionIcon } from '@mantine/core';
import { IconPlus, IconTrash, IconMusic, IconImage } from '@tabler/icons-react';
import { memo, useState, useCallback } from 'react';
import type { RecordingMetadata, MetadataPreset } from '../../types/audio.types';

type MetadataFormProps = {
  metadata: RecordingMetadata;
  onChange: (metadata: RecordingMetadata) => void;
  presets?: MetadataPreset[];
  showPresetButtons?: boolean;
};

const genreOptions = [
  'Electronic', 'House', 'Techno', 'Trance', 'Dubstep', 'Drum & Bass',
  'Hip Hop', 'Rap', 'R&B', 'Pop', 'Rock', 'Alternative', 'Indie',
  'Jazz', 'Blues', 'Classical', 'Country', 'Folk', 'Reggae',
  'Podcast', 'Spoken Word', 'Voice Memo', 'DJ Mix', 'Live Performance',
  'Demo', 'Other'
];

export const MetadataForm = memo<MetadataFormProps>(
  ({ metadata, onChange, presets = [], showPresetButtons = true }) => {
    const [customTagKey, setCustomTagKey] = useState('');
    const [customTagValue, setCustomTagValue] = useState('');

    const updateField = useCallback((field: keyof RecordingMetadata, value: any) => {
      onChange({
        ...metadata,
        [field]: value || undefined,
      });
    }, [metadata, onChange]);

    const updateCustomTags = useCallback((tags: Record<string, string>) => {
      onChange({
        ...metadata,
        custom_tags: tags,
      });
    }, [metadata, onChange]);

    const addCustomTag = useCallback(() => {
      if (!customTagKey.trim() || !customTagValue.trim()) return;
      
      const newTags = { ...(metadata.custom_tags || {}), [customTagKey]: customTagValue };
      updateCustomTags(newTags);
      setCustomTagKey('');
      setCustomTagValue('');
    }, [customTagKey, customTagValue, metadata.custom_tags, updateCustomTags]);

    const removeCustomTag = useCallback((key: string) => {
      const newTags = { ...(metadata.custom_tags || {}) };
      delete newTags[key];
      updateCustomTags(newTags);
    }, [metadata.custom_tags, updateCustomTags]);

    const applyPreset = useCallback((preset: MetadataPreset) => {
      // Preserve any existing custom tags and technical fields
      const preservedFields = {
        encoder: metadata.encoder,
        encoding_date: metadata.encoding_date,
        sample_rate: metadata.sample_rate,
        bitrate: metadata.bitrate,
        duration_seconds: metadata.duration_seconds,
        custom_tags: metadata.custom_tags,
      };

      onChange({
        ...preset.metadata,
        ...preservedFields,
      });
    }, [metadata, onChange]);

    return (
      <Stack gap="md">
        {/* Metadata Presets */}
        {showPresetButtons && presets.length > 0 && (
          <>
            <Group justify="space-between" align="center">
              <Text size="sm" fw={500}>Metadata Presets</Text>
            </Group>
            <Group gap="xs">
              {presets.map((preset) => (
                <Button
                  key={preset.name}
                  size="xs"
                  variant="light"
                  leftSection={<IconMusic size={14} />}
                  onClick={() => applyPreset(preset)}
                >
                  {preset.name}
                </Button>
              ))}
            </Group>
            <Divider />
          </>
        )}

        {/* Core Metadata Fields */}
        <Title order={6}>Basic Information</Title>
        <Grid>
          <Grid.Col span={6}>
            <TextInput
              label="Title"
              value={metadata.title || ''}
              onChange={(e) => updateField('title', e.target.value)}
              placeholder="Track/recording title"
            />
          </Grid.Col>
          <Grid.Col span={6}>
            <TextInput
              label="Artist"
              value={metadata.artist || ''}
              onChange={(e) => updateField('artist', e.target.value)}
              placeholder="Primary artist"
            />
          </Grid.Col>
          <Grid.Col span={6}>
            <TextInput
              label="Album"
              value={metadata.album || ''}
              onChange={(e) => updateField('album', e.target.value)}
              placeholder="Album or collection name"
            />
          </Grid.Col>
          <Grid.Col span={6}>
            <Select
              label="Genre"
              value={metadata.genre || ''}
              onChange={(value) => updateField('genre', value)}
              data={genreOptions}
              searchable
              clearable
              placeholder="Select or type genre"
            />
          </Grid.Col>
        </Grid>

        {/* Extended Fields */}
        <Title order={6} mt="md">Extended Information</Title>
        <Grid>
          <Grid.Col span={6}>
            <TextInput
              label="Album Artist"
              value={metadata.album_artist || ''}
              onChange={(e) => updateField('album_artist', e.target.value)}
              placeholder="Various Artists, etc."
            />
          </Grid.Col>
          <Grid.Col span={6}>
            <TextInput
              label="Composer"
              value={metadata.composer || ''}
              onChange={(e) => updateField('composer', e.target.value)}
              placeholder="Music composer"
            />
          </Grid.Col>
          <Grid.Col span={3}>
            <NumberInput
              label="Track #"
              value={metadata.track_number || ''}
              onChange={(value) => updateField('track_number', typeof value === 'number' ? value : undefined)}
              min={1}
              max={999}
              placeholder="1"
            />
          </Grid.Col>
          <Grid.Col span={3}>
            <NumberInput
              label="Total Tracks"
              value={metadata.total_tracks || ''}
              onChange={(value) => updateField('total_tracks', typeof value === 'number' ? value : undefined)}
              min={1}
              max={999}
              placeholder="10"
            />
          </Grid.Col>
          <Grid.Col span={3}>
            <NumberInput
              label="Year"
              value={metadata.year || ''}
              onChange={(value) => updateField('year', typeof value === 'number' ? value : undefined)}
              min={1900}
              max={2100}
              placeholder="2024"
            />
          </Grid.Col>
          <Grid.Col span={3}>
            <NumberInput
              label="BPM"
              value={metadata.bpm || ''}
              onChange={(value) => updateField('bpm', typeof value === 'number' ? value : undefined)}
              min={1}
              max={999}
              placeholder="128"
            />
          </Grid.Col>
          <Grid.Col span={6}>
            <TextInput
              label="Copyright"
              value={metadata.copyright || ''}
              onChange={(e) => updateField('copyright', e.target.value)}
              placeholder="© 2024 Artist Name"
            />
          </Grid.Col>
          <Grid.Col span={6}>
            <TextInput
              label="ISRC"
              value={metadata.isrc || ''}
              onChange={(e) => updateField('isrc', e.target.value)}
              placeholder="International Standard Recording Code"
            />
          </Grid.Col>
        </Grid>

        <Textarea
          label="Comment / Notes"
          value={metadata.comment || ''}
          onChange={(e) => updateField('comment', e.target.value)}
          placeholder="Additional notes about this recording..."
          rows={3}
        />

        {/* Custom Tags */}
        <Title order={6} mt="md">Custom Tags</Title>
        {metadata.custom_tags && Object.keys(metadata.custom_tags).length > 0 && (
          <Stack gap="xs">
            {Object.entries(metadata.custom_tags).map(([key, value]) => (
              <Group key={key} justify="space-between" align="center">
                <Text size="sm">
                  <strong>{key}:</strong> {value}
                </Text>
                <ActionIcon
                  size="sm"
                  variant="subtle"
                  color="red"
                  onClick={() => removeCustomTag(key)}
                >
                  <IconTrash size={14} />
                </ActionIcon>
              </Group>
            ))}
          </Stack>
        )}

        <Group align="end">
          <TextInput
            label="Tag Name"
            value={customTagKey}
            onChange={(e) => setCustomTagKey(e.target.value)}
            placeholder="e.g., mood, energy_level"
            style={{ flex: 1 }}
          />
          <TextInput
            label="Tag Value"
            value={customTagValue}
            onChange={(e) => setCustomTagValue(e.target.value)}
            placeholder="e.g., energetic, chill"
            style={{ flex: 1 }}
          />
          <Button
            leftSection={<IconPlus size={16} />}
            onClick={addCustomTag}
            disabled={!customTagKey.trim() || !customTagValue.trim()}
          >
            Add Tag
          </Button>
        </Group>

        {/* Technical Information (Read-Only) */}
        {(metadata.encoder || metadata.sample_rate || metadata.bitrate) && (
          <>
            <Divider />
            <Title order={6}>Technical Information</Title>
            <Grid>
              {metadata.encoder && (
                <Grid.Col span={6}>
                  <TextInput
                    label="Encoder"
                    value={metadata.encoder}
                    readOnly
                    variant="filled"
                  />
                </Grid.Col>
              )}
              {metadata.sample_rate && (
                <Grid.Col span={3}>
                  <TextInput
                    label="Sample Rate"
                    value={`${metadata.sample_rate} Hz`}
                    readOnly
                    variant="filled"
                  />
                </Grid.Col>
              )}
              {metadata.bitrate && (
                <Grid.Col span={3}>
                  <TextInput
                    label="Bitrate"
                    value={`${metadata.bitrate} kbps`}
                    readOnly
                    variant="filled"
                  />
                </Grid.Col>
              )}
              {metadata.duration_seconds && (
                <Grid.Col span={6}>
                  <TextInput
                    label="Duration"
                    value={`${Math.floor(metadata.duration_seconds / 60)}:${String(Math.floor(metadata.duration_seconds % 60)).padStart(2, '0')}`}
                    readOnly
                    variant="filled"
                  />
                </Grid.Col>
              )}
            </Grid>
          </>
        )}
      </Stack>
    );
  }
);

MetadataForm.displayName = 'MetadataForm';