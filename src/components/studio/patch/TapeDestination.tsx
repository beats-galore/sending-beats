import { Box, Group, Stack, Text } from '@mantine/core';

import { TAPE_TARGET_KEY } from '../../../services/patch-color-service';
import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { asBytes, asClock } from '../format';
import { useNodeDrag, useNodeFront } from '../hooks/use-node-drag';
import { useNodeResize, useNodeSize } from '../hooks/use-node-resize';
import { useTapeTransport } from '../hooks/use-tape-transport';
import { ExpandToggle } from '../primitives/ExpandToggle';
import { NodeCard } from '../primitives/NodeCard';
import { PortDot } from '../primitives/PortDot';
import { StatusDot } from '../primitives/StatusDot';
import { tapeExpandedFor, tapeSize } from './patch-geometry';
import type { NodeRect } from './patch-layout';
import { TapeInspector } from './TapeInspector';

const { destination } = layout;

type TapeDestinationProps = {
  /** Box in canvas coordinates, with anything the user arranged applied. */
  rect: NodeRect;
  /** Holds the ring. Does not open the node. */
  selected: boolean;
};

// Unnumbered and uncoloured by hand: there is only one tape, so it keeps the red
// it turns while recording. See `reservedPatchColor`.
/** The recorder, as seen from the patchbay. Opens in place to show its output settings. */
export const TapeDestination = ({ rect, selected }: TapeDestinationProps) => {
  const select = useStudioStore((state) => state.select);
  const tape = useTapeTransport();
  const grab = useNodeDrag(TAPE_TARGET_KEY, rect);
  const resize = useNodeResize(TAPE_TARGET_KEY, rect, tapeSize(false));
  const setSize = useNodeSize(TAPE_TARGET_KEY);
  const { front, bringToFront } = useNodeFront(TAPE_TARGET_KEY);
  const expanded = tapeExpandedFor(rect);

  const fileName = tape.filePath?.split('/').pop();

  return (
    <NodeCard
      position={rect}
      selected={selected}
      raised={front}
      borderColor={selected ? color.acc : color.line}
      onPress={bringToFront}
      onClick={() => select({ kind: 'tape' })}
      onGrab={grab}
      onResize={resize}
      ports={
        <PortDot
          tone={tape.isRecording ? 'hot' : 'dead'}
          side="left"
          top={destination.tapePortOffset}
        />
      }
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
          <ExpandToggle expanded={expanded} onToggle={() => setSize(tapeSize(!expanded))} />
        </>
      }
      bodyStyle={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 12 }}
    >
      <Group gap="xl" align="center" wrap="nowrap" h={expanded ? undefined : '100%'}>
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

      {expanded && <TapeInspector tape={tape} />}
    </NodeCard>
  );
};
