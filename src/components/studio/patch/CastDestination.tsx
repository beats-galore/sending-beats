import { Box, Group, SimpleGrid, Stack, Text } from '@mantine/core';

import { STREAM_TARGET_KEY } from '../../../services/patch-color-service';
import {
  selectedCastConfiguration,
  useCastConfigurationStore,
} from '../../../stores/cast-configuration-store';
import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import { bytesSent } from '../../../hooks/use-streaming-status';
import { castAddress } from '../../../types/cast.types';
import { asBytes, asElapsed } from '../format';
import { useCastDestination } from '../hooks/use-cast-destination';
import { useListenerStats } from '../hooks/use-listener-stats';
import { useNodeDrag, useNodeFront } from '../hooks/use-node-drag';
import { useNodeResize, useNodeRung, useUnshrink } from '../hooks/use-node-resize';
import { useStreamTransport } from '../hooks/use-stream-transport';
import { DeleteButton } from '../primitives/DeleteButton';
import { ExpandToggle } from '../primitives/ExpandToggle';
import { NodeCard } from '../primitives/NodeCard';
import { PortDot } from '../primitives/PortDot';
import { SectionLabel } from '../primitives/SectionLabel';
import { StatRow } from '../primitives/StatRow';
import { StatusDot } from '../primitives/StatusDot';
import { CastInspector } from './CastInspector';
import { castExpansionFor, castSize, NodeExpansion, nextExpansion } from './patch-geometry';
import type { NodeRect } from './patch-layout';
import { SourceTiles } from './SourceTiles';

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
  const station = useCastConfigurationStore(selectedCastConfiguration);
  const cast = useCastDestination();
  const removeCastTarget = useCastConfigurationStore((state) => state.removeTarget);
  const { isLive, isBusy, toggle, status, uptimeSeconds } = useStreamTransport();
  const isImpulse = station?.protocol === 'impulse';
  // Impulse publishes no listener counts: delivery happens at the edge cache, so
  // nothing in the path a listener takes reports back.
  const listeners = useListenerStats(isLive, !isImpulse);
  const grab = useNodeDrag(STREAM_TARGET_KEY, rect);
  const resize = useNodeResize(STREAM_TARGET_KEY, rect, castSize('compact'));
  const setRung = useNodeRung(STREAM_TARGET_KEY);
  const { front, bringToFront } = useNodeFront(STREAM_TARGET_KEY);

  const expansion = castExpansionFor(rect);
  const next = nextExpansion(expansion, NodeExpansion);
  const unshrink = useUnshrink(STREAM_TARGET_KEY, expansion === 'compact');

  const bitrate = status?.bitrate_info.current_bitrate ?? station?.bitrateKbps ?? 0;
  const sent = bytesSent(status);
  // The station the transmitter is pointed at, so the node says where the mix
  // is actually going rather than what was last typed somewhere else.
  const target = station ? castAddress(station) : 'no station';
  // Named for the transmitter, so a glance at the patchbay says which of the two
  // this broadcast is going out over.
  const transmitter = isImpulse ? 'IMPULSE' : 'ICECAST';
  const quality = isImpulse
    ? `${station.segmentMs / 1000}s segments`
    : station?.variableBitrate
      ? `VBR q${station.vbrQuality}`
      : 'CBR';

  return (
    <NodeCard
      position={rect}
      selected={selected}
      raised={front}
      borderColor={isLive ? color.hotBorder : selected ? color.acc : color.line}
      headerSurface={isLive ? 'hotBg' : 'bgRaised'}
      onPress={bringToFront}
      onClick={() => {
        select({ kind: 'cast' });
        unshrink();
      }}
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
            {transmitter} · {isLive ? 'ON AIR' : 'OFFLINE'}
          </Text>
          <Text size="2xs" c={color.textDim}>
            MP3 {bitrate}
          </Text>
          {/* Takes the station off this patch, not out of the studio. Its
              routing is left where it is, so putting it back finds the sources
              it had rather than a destination to wire up again. */}
          {cast && !isLive && (
            <DeleteButton
              onDelete={() => void removeCastTarget(cast.castConfigurationId)}
              title={`Take ${cast.name} off this patch`}
            />
          )}
          <ExpandToggle
            grows={NodeExpansion.indexOf(next) > NodeExpansion.indexOf(expansion)}
            onToggle={() => setRung(next)}
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

          {/* The broadcast is routed to like any other destination now: its
              output identity comes from the station rather than from the
              running stream, so this can be wired up off air. */}
          {cast && (
            <Group gap="md" wrap="nowrap" w="100%">
              <SectionLabel tracking="tight">FROM</SectionLabel>
              <SourceTiles deviceId={cast.deviceId} />
            </Group>
          )}

          {expansion === 'expanded' && (
            <CastInspector isLive={isLive} isBusy={isBusy} onToggle={() => void toggle()} />
          )}
        </Stack>
      )}
    </NodeCard>
  );
};
