import { Text } from '@mantine/core';

import { outputTargetKey } from '../../../services/patch-color-service';
import type { DestinationRole } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { useNodeDrag, useNodeFront } from '../hooks/use-node-drag';
import { useNodeResize, useNodeSize, useUnshrink } from '../hooks/use-node-resize';
import type { PatchOutput } from '../hooks/use-patch-outputs';
import { DeleteButton } from '../primitives/DeleteButton';
import { ExpandToggle } from '../primitives/ExpandToggle';
import { NodeCard } from '../primitives/NodeCard';
import { Pill } from '../primitives/Pill';
import { PortDot } from '../primitives/PortDot';
import { OutputBody } from './OutputBody';
import { nextExpansion, OutputExpansion, outputExpansionFor, outputSize } from './patch-geometry';
import type { NodeRect } from './patch-layout';
import { PatchBadge } from './PatchBadge';

const { destination } = layout;

const ROLE_PILL: Record<DestinationRole, 'accent' | 'warn' | 'neutral'> = {
  MAIN: 'accent',
  CUE: 'warn',
  SEND: 'neutral',
};

/** Cue sends are flagged amber on the patch point; everything else reads as signal. */
const ROLE_PORT: Record<DestinationRole, 'accent' | 'warn'> = {
  MAIN: 'accent',
  CUE: 'warn',
  SEND: 'accent',
};

type OutputDestinationProps = {
  output: PatchOutput;
  /** Box in canvas coordinates, with anything the user arranged applied. */
  rect: NodeRect;
  /** Where this sits in the destination column, for its number and colour */
  position: number;
  options: { value: string; label: string }[];
  /** Why the last attempt to re-point this destination failed, if it did */
  switchError: string | null;
  onSelect: (deviceId: string) => void;
  onChangeDevice: (oldDeviceId: string, newDeviceId: string) => void;
  onCycleRole: (deviceId: string) => void;
  onGainChange: (deviceId: string, gainDb: number) => void;
  onRemove: (deviceId: string) => void;
};

/**
 * One hardware output the master sum can feed.
 *
 * Laid out like a source: the name in the title bar, what it is patched to
 * inside. The device picker used to fill the title bar, which left no bare
 * strip anywhere along it to pick the node up by.
 */
export const OutputDestination = ({
  output,
  rect,
  position,
  options,
  switchError,
  onSelect,
  onChangeDevice,
  onCycleRole,
  onGainChange,
  onRemove,
}: OutputDestinationProps) => {
  // A destination the configuration still points at but which never connected.
  // Without this it renders as live and the mix appears to be going somewhere.
  const unavailable = output.unavailableReason !== null;
  const targetKey = outputTargetKey(output.id);
  const grab = useNodeDrag(targetKey, rect);
  const resize = useNodeResize(targetKey, rect, outputSize('compact'));
  const setSize = useNodeSize(targetKey);
  const { front, bringToFront } = useNodeFront(targetKey);

  // A destination has nothing to open into, so it only shrinks: the device
  // picker and the trim give way, and the tiles saying which mixes feed it stay.
  const expansion = outputExpansionFor(rect);
  const compact = expansion === 'compact';
  const next = nextExpansion(expansion, OutputExpansion);
  const unshrink = useUnshrink(targetKey, compact, outputSize('collapsed'));

  return (
    <NodeCard
      position={rect}
      raised={front}
      onPress={bringToFront}
      // A destination has nothing to select, so it is only clickable while it
      // is shrunk — where the click is asking to see it rather than to pick it.
      onClick={compact ? unshrink : undefined}
      onGrab={grab}
      onResize={resize}
      borderColor={unavailable ? color.hotBorder : output.live ? color.line : color.dash}
      dimmed={!output.live}
      ports={
        <PortDot
          tone={unavailable ? 'dead' : output.live ? ROLE_PORT[output.role] : 'dead'}
          side="left"
          top={destination.outputPortOffset}
        />
      }
      header={
        <>
          {/* Not dimmed for being idle. The engine drives one master output at
              a time, so every destination but one is `live: false` — greying on
              that would hide the colour almost everywhere it is needed. */}
          <PatchBadge
            targetKey={targetKey}
            position={position}
            dimmed={unavailable}
            label="DESTINATION COLOUR"
          />
          <Text
            ff="var(--mantine-font-family-headings)"
            fw={600}
            fz="md"
            truncate
            c={unavailable ? color.hotText : color.text}
            style={{ flex: 1, minWidth: 0, letterSpacing: layout.tracking.tight }}
          >
            {output.name}
          </Text>
          {switchError ? (
            <Pill tone="hot" title={switchError}>
              FAILED
            </Pill>
          ) : unavailable ? (
            <Pill tone="hot" title={output.unavailableReason ?? undefined}>
              OFFLINE
            </Pill>
          ) : (
            <Pill tone={ROLE_PILL[output.role]} onClick={() => onCycleRole(output.id)}>
              {output.role}
            </Pill>
          )}
          <ExpandToggle
            grows={OutputExpansion.indexOf(next) > OutputExpansion.indexOf(expansion)}
            onToggle={() => setSize(outputSize(next))}
          />
          <DeleteButton
            onDelete={() => onRemove(output.id)}
            title={`Remove ${output.name} from the mix`}
          />
        </>
      }
      bodyStyle={{
        padding: '10px 11px',
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        overflow: 'hidden',
      }}
    >
      <OutputBody
        output={output}
        compact={compact}
        options={options}
        onSelect={onSelect}
        onChangeDevice={onChangeDevice}
        onGainChange={onGainChange}
      />
    </NodeCard>
  );
};
