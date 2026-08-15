import { Box, Group, SimpleGrid, Stack, Text } from '@mantine/core';

import { STREAM_TARGET_KEY } from '../../../services/patch-color-service';
import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import { asBytes, asElapsed } from '../format';
import { useListenerStats } from '../hooks/use-listener-stats';
import { useNodeDrag, useNodeFront } from '../hooks/use-node-drag';
import { useNodeResize, useNodeSize } from '../hooks/use-node-resize';
import { useStreamTransport } from '../hooks/use-stream-transport';
import { ExpandToggle } from '../primitives/ExpandToggle';
import { NodeCard } from '../primitives/NodeCard';
import { PortDot } from '../primitives/PortDot';
import { StatRow } from '../primitives/StatRow';
import { StatusDot } from '../primitives/StatusDot';
import { CastInspector } from './CastInspector';
import { castExpansionFor, castSize, NodeExpansion, nextExpansion } from './patch-geometry';
import type { NodeRect } from './patch-layout';

const { destination } = layout;

type CastDestinationProps = {
  /** Box in canvas coordinates, with anything the user arranged applied. */
  rect: NodeRect;
  /** Holds the ring. Does not open the node. */
  selected: boolean;
};

// Unnumbered and uncoloured by hand: there is only one broadcast, so its colour
// is reserved rather than picked. See `reservedPatchColor`.
/** The stream, as seen from the patchbay. Opens in place to show the transmitter. */
export const CastDestination = ({ rect, selected }: CastDestinationProps) => {
  const select = useStudioStore((state) => state.select);
  const stream = useStudioStore((state) => state.stream);
  const { isLive, isBusy, toggle, status, uptimeSeconds } = useStreamTransport();
  const listeners = useListenerStats(isLive);
  const grab = useNodeDrag(STREAM_TARGET_KEY, rect);
  const resize = useNodeResize(STREAM_TARGET_KEY, rect, castSize('compact'));
  const setSize = useNodeSize(STREAM_TARGET_KEY);
  const { front, bringToFront } = useNodeFront(STREAM_TARGET_KEY);

  const expansion = castExpansionFor(rect);
  const next = nextExpansion(expansion, NodeExpansion);

  const bitrate = status?.bitrate_info.current_bitrate ?? stream.bitrate;
  const sent = status?.icecast_stats?.bytes_sent;
  const target = `${stream.host}:${stream.port}${stream.mount}`;
  const quality = stream.variableBitrate ? `VBR q${stream.vbrQuality}` : 'CBR';

  return (
    <NodeCard
      position={rect}
      selected={selected}
      raised={front}
      borderColor={isLive ? color.hotBorder : selected ? color.acc : color.line}
      headerSurface={isLive ? 'hotBg' : 'bgRaised'}
      onPress={bringToFront}
      onClick={() => select({ kind: 'cast' })}
      onGrab={grab}
      onResize={resize}
      ports={
        <PortDot tone={isLive ? 'hot' : 'dead'} side="left" top={destination.castPortOffset} />
      }
      header={
        <>
          <StatusDot tone={isLive ? 'hot' : 'inert'} />
          <Text
            ff="var(--mantine-font-family-headings)"
            fw={600}
            fz="lg"
            c={isLive ? color.hotText : color.textDim}
            style={{ flex: 1, letterSpacing: layout.tracking.label }}
          >
            {isLive ? 'ICECAST · ON AIR' : 'ICECAST · OFFLINE'}
          </Text>
          <Text size="2xs" c={color.textDim}>
            MP3 {bitrate}
          </Text>
          <ExpandToggle
            grows={NodeExpansion.indexOf(next) > NodeExpansion.indexOf(expansion)}
            onToggle={() => setSize(castSize(next))}
          />
        </>
      }
      bodyStyle={{ padding: 12 }}
    >
      {/* Shrunk, the broadcast is where it is sending and at what rate — enough
          to tell one configured transmitter from another without opening it. */}
      {expansion === 'compact' ? (
        <Group gap="sm" wrap="nowrap" h="100%" align="center">
          <Text size="xs" truncate style={{ flex: 1, minWidth: 0 }}>
            {target}
          </Text>
          <Text size="2xs" c={color.textFaint} style={{ flex: 'none' }}>
            {bitrate} kbps · {quality}
          </Text>
        </Group>
      ) : (
        <Stack gap="md" h="100%">
          <Box
            px="md"
            py="sm"
            style={{
              background: color.bg,
              border: border(),
              borderRadius: 'var(--mantine-radius-sm)',
            }}
          >
            <Text size="xs" truncate>
              {target}
            </Text>
          </Box>

          <SimpleGrid cols={2} spacing="sm" verticalSpacing="sm">
            <StatRow label="LISTENERS" tone={color.acc}>
              {listeners.current ?? '—'}
            </StatRow>
            <StatRow label="PEAK">{listeners.peak ?? '—'}</StatRow>
            <StatRow label="UPTIME">{isLive ? asElapsed(uptimeSeconds) : '—'}</StatRow>
            <StatRow label="SENT">{sent ? asBytes(sent) : '—'}</StatRow>
          </SimpleGrid>

          {expansion === 'expanded' && (
            <CastInspector isLive={isLive} isBusy={isBusy} onToggle={() => void toggle()} />
          )}
        </Stack>
      )}
    </NodeCard>
  );
};
