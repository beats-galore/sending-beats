import { SimpleGrid, TextInput } from '@mantine/core';
import { useCallback, useState } from 'react';

import type { RecordingMetadata } from '../../../types/audio.types';
import type { useTapeTransport } from '../hooks/use-tape-transport';
import { ActionButton } from '../primitives/ActionButton';
import { Panel } from '../primitives/Panel';
import { SectionLabel } from '../primitives/SectionLabel';


const FIELDS = [
  { key: 'title', label: 'TITLE' },
  { key: 'artist', label: 'ARTIST' },
  { key: 'album', label: 'ALBUM' },
  { key: 'genre', label: 'GENRE' },
] as const;

type TapeTagsProps = {
  tape: ReturnType<typeof useTapeTransport>;
};

/** Tags written into the take being recorded. */
export const TapeTags = ({ tape }: TapeTagsProps) => {
  const [metadata, setMetadata] = useState<RecordingMetadata>(tape.config?.metadata ?? {});
  const [applied, setApplied] = useState(false);

  const apply = useCallback(() => {
    tape.updateConfig({ metadata });
    if (tape.isRecording) {
      void tape.actions.updateSessionMetadata(metadata);
    }
    setApplied(true);
  }, [metadata, tape]);

  return (
    <Panel
      p="3xl"
      title={<SectionLabel tracking="widest">TAGS FOR THIS TAKE</SectionLabel>}
      action={
        <ActionButton tone="ghost" padding="0" size="xs" onClick={apply}>
          {applied ? 'APPLIED ✓' : 'APPLY TO TAKE'}
        </ActionButton>
      }
    >
      <SimpleGrid cols={2} spacing="xl" verticalSpacing="lg">
        {FIELDS.map((field) => (
          <TextInput
            key={field.key}
            label={field.label}
            value={metadata[field.key] ?? ''}
            onChange={(event) => {
              setMetadata((current) => ({ ...current, [field.key]: event.currentTarget.value }));
              setApplied(false);
            }}
          />
        ))}
      </SimpleGrid>
    </Panel>
  );
};
