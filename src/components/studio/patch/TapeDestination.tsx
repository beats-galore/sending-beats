import { Box, Group, Stack, Text } from '@mantine/core';

import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { asBytes, asClock } from '../format';
import { useTapeTransport } from '../hooks/use-tape-transport';
import { NodeCard } from '../primitives/NodeCard';
import { PortDot } from '../primitives/PortDot';
import { StatusDot } from '../primitives/StatusDot';

const { destination } = layout;

/** The recorder, as seen from the patchbay. Opens the TAPE view. */
export const TapeDestination = () => {
  const setView = useStudioStore((state) => state.setView);
  const tape = useTapeTransport();

  const fileName = tape.filePath?.split('/').pop();

  return (
    <NodeCard
      position={{
        left: destination.x,
        top: destination.tapeTop,
        width: destination.width,
        height: destination.tapeHeight,
      }}
      onClick={() => setView('tape')}
      ports={<PortDot tone={tape.isRecording ? 'hot' : 'dead'} side="left" top={63} />}
      header={
        <>
          <StatusDot tone={tape.isRecording ? 'hot' : 'inert'} />
          <Text
            ff="var(--mantine-font-family-headings)"
            fw={600}
            fz="lg"
            style={{ flex: 1, letterSpacing: layout.tracking.label }}
          >
            {tape.isRecording ? 'TAPE · RECORDING' : 'TAPE · READY'}
          </Text>
          <Text size="2xs" c={color.textDim}>
            {tape.config?.name ?? '—'}
          </Text>
        </>
      }
      bodyStyle={{ padding: 12 }}
    >
      <Group gap="xl" align="center" wrap="nowrap" h="100%">
        <Box
          onClick={(event) => {
            event.stopPropagation();
            void tape.toggle();
          }}
          style={{
            width: 40,
            height: 40,
            flex: 'none',
            borderRadius: '50%',
            cursor: 'pointer',
            background: tape.isRecording ? color.hot : color.textFaintest,
            boxShadow: `0 0 0 4px ${color.hotBg}`,
          }}
        />
        <Stack gap="2xs" style={{ flex: 1, minWidth: 0 }}>
          <Text fz="3xl" fw={600} style={{ letterSpacing: layout.tracking.tight }}>
            {asClock(tape.elapsedSeconds)}
          </Text>
          <Text size="2xs" c={color.textDim} truncate>
            {tape.isRecording && fileName
              ? `${fileName} · ${asBytes(tape.fileSizeBytes)}`
              : 'ready — nothing on tape yet'}
          </Text>
        </Stack>
      </Group>
    </NodeCard>
  );
};
