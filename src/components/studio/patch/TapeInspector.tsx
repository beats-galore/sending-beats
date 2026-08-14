import { Group, NativeSelect, Stack, Switch, Text, TextInput } from '@mantine/core';
import { invoke } from '@tauri-apps/api/core';
import { useCallback } from 'react';

import { border, color } from '../../../theme/tokens';
import type { useTapeTransport } from '../hooks/use-tape-transport';
import { ActionButton } from '../primitives/ActionButton';
import { FORMAT_CHOICES, formatFromLabel, formatLabel } from '../tape/recording-format';

const SPLIT_MINUTES = 60;

type TapeInspectorProps = {
  tape: ReturnType<typeof useTapeTransport>;
};

/** Where takes land and in what shape, revealed inside the tape node. */
export const TapeInspector = ({ tape }: TapeInspectorProps) => {
  const { config, updateConfig } = tape;

  const handleBrowse = useCallback(() => {
    void invoke<string | null>('select_recording_directory').then((directory) => {
      if (directory) {
        updateConfig({ output_directory: directory });
      }
    });
  }, [updateConfig]);

  return (
    <Stack
      gap="lg"
      pt="lg"
      style={{ borderTop: border() }}
      onClick={(event) => event.stopPropagation()}
    >
      <NativeSelect
        label="FORMAT"
        value={formatLabel(config?.format)}
        onChange={(event) => updateConfig({ format: formatFromLabel(event.currentTarget.value) })}
        data={[...FORMAT_CHOICES]}
      />

      <Group gap="sm" wrap="nowrap" align="flex-end">
        <TextInput
          label="FOLDER"
          value={config?.output_directory ?? ''}
          onChange={(event) => updateConfig({ output_directory: event.currentTarget.value })}
          style={{ flex: 1, minWidth: 0 }}
        />
        <ActionButton tone="ghost" padding="9px 14px" size="xs" onClick={handleBrowse}>
          BROWSE
        </ActionButton>
      </Group>

      <div>
        <TextInput
          label="FILENAME TEMPLATE"
          value={config?.filename_template ?? ''}
          onChange={(event) => updateConfig({ filename_template: event.currentTarget.value })}
        />
        <Text size="2xs" c={color.textFaintest} mt="2xs">
          {'{timestamp} · {title} · {artist} · {album} · {genre}'}
        </Text>
      </div>

      <Switch
        checked={Boolean(config?.split_on_interval_minutes)}
        onChange={(event) =>
          updateConfig({
            split_on_interval_minutes: event.currentTarget.checked ? SPLIT_MINUTES : undefined,
          })
        }
        label={`Split file every ${SPLIT_MINUTES} minutes`}
      />
    </Stack>
  );
};
