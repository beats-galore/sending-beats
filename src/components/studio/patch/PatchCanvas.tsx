import { Box, Text } from '@mantine/core';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { useChannelsData } from '../../../hooks';
import { useMixerStore } from '../../../stores';
import { orderedBuses, useBusStore } from '../../../stores/bus-store';
import { usePatchColorStore } from '../../../stores/patch-color-store';
import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { MAIN_BUS_ID } from '../../../types/bus.types';
import { useChannelCardVariants } from '../hooks/use-channel-card-variants';
import { useChannelDevices } from '../hooks/use-channel-devices';
import { useFocusedNode } from '../hooks/use-focused-node';
import { usePatchOutputs } from '../hooks/use-patch-outputs';
import { DashedTarget } from '../primitives/DashedTarget';
import { PortDot } from '../primitives/PortDot';
import { SectionLabel } from '../primitives/SectionLabel';
import { AddDestination } from './AddDestination';
import { BusNode } from './BusNode';
import { CableLayer } from './CableLayer';
import type { Cable } from './CableLayer';
import { CastDestination } from './CastDestination';
import { ChannelNode } from './ChannelNode';
import { resolveDestination } from './destination-target';
import { OutputDestination } from './OutputDestination';
import {
  busPort,
  busTop,
  cablePath,
  canvasHeight,
  channelPort,
  channelTop,
  extraTop,
  outputTop,
  sourceStackHeight,
  tapeTop,
} from './patch-geometry';
import type { ChannelLayout, DestinationFocus } from './patch-geometry';
import { TapeDestination } from './TapeDestination';

const { source, bus, destination, canvas } = layout;

const CAST_PORT_OFFSET = 83;
const TAPE_PORT_OFFSET = 63;
const OUTPUT_PORT_OFFSET = 24;

/** The wiring diagram: sources on the left, the sum in the middle, destinations on the right. */
export const PatchCanvas = () => {
  const { channels } = useChannelsData();
  const addChannel = useMixerStore((state) => state.addChannel);
  const {
    outputs,
    available,
    optionsFor,
    selectOutput,
    changeOutput,
    removeOutput,
    cycleOutputRole,
    setOutputGain,
  } = usePatchOutputs();

  // Colours belong to a configuration, so switching patches has to fetch the
  // new one's rather than leave the previous patch's colours on screen.
  const activeConfigurationId = useMixerStore((state) => state.activeSession?.configuration.id);
  const loadPatchColors = usePatchColorStore((state) => state.load);
  useEffect(() => {
    void loadPatchColors();
  }, [loadPatchColors, activeConfigurationId]);

  // Routing is stored per configuration too, and restoring it is what moves
  // devices off the main bus they each registered onto.
  const loadBuses = useBusStore((state) => state.load);
  useEffect(() => {
    void loadBuses();
  }, [loadBuses, activeConfigurationId]);

  const storedBuses = useBusStore((state) => state.buses);
  const buses = useMemo(() => orderedBuses(storedBuses), [storedBuses]);
  const setBusGain = useBusStore((state) => state.setGain);
  const channelDevices = useChannelDevices();
  const outputIds = useMemo(() => outputs.map((output) => output.id), [outputs]);

  // Kept per destination rather than in the store: a destination that refuses a
  // device is a problem with that node, and routing it through the mixer's
  // `error` would replace the whole patchbay with an error page.
  const [outputErrors, setOutputErrors] = useState<Record<string, string>>({});

  const handleChangeOutput = useCallback(
    (oldDeviceId: string, newDeviceId: string) => {
      void changeOutput(oldDeviceId, newDeviceId).then((failure) => {
        setOutputErrors((previous) => {
          const next = { ...previous };
          delete next[oldDeviceId];
          if (failure) {
            next[oldDeviceId] = failure;
          }
          return next;
        });
      });
    },
    [changeOutput]
  );

  const clearSelection = useStudioStore((state) => state.clearSelection);
  const focused = useFocusedNode();
  const selectedId = focused?.kind === 'channel' ? focused.channelId : null;
  const destinationFocus: DestinationFocus =
    focused?.kind === 'cast' || focused?.kind === 'tape' ? focused.kind : null;

  const variants = useChannelCardVariants();

  const layouts = channels.map<ChannelLayout>((channel) => ({
    variant: variants[channel.id] ?? 'device',
    expansion:
      channel.id !== selectedId ? 'collapsed' : channel.effects_enabled ? 'effects' : 'inspector',
  }));

  // Main is open whenever nothing else is, so the column is never all shut and
  // the mix a device falls back to is the one on show by default. Expanding
  // another bus closes it; clicking away from that bus hands the focus back.
  const expandedBusId = focused?.kind === 'bus' ? focused.busId : MAIN_BUS_ID;
  const busExpansions = useMemo(
    () => buses.map((busEntry) => busEntry.id === expandedBusId),
    [buses, expandedBusId]
  );

  const height = canvasHeight(layouts, destinationFocus, outputs.length, 0, false, busExpansions);

  // One cable per membership rather than per card: a source feeding two buses
  // draws two cables, which is what makes a split visible on the canvas.
  const cables = useMemo<Cable[]>(() => {
    const sourceCables: Cable[] = buses.flatMap((busEntry, busIndex) =>
      busEntry.inputs.flatMap((deviceId, portIndex) => {
        const channel = channelDevices.find(
          (candidate) => candidate.deviceIdentifier === deviceId
        );
        if (!channel) {
          return [];
        }

        return [
          {
            id: `in-${busEntry.id}-${deviceId}`,
            path: cablePath(
              channelPort(channel.index, layouts),
              busPort(busIndex, busExpansions, portIndex, busEntry.inputs.length, 'in')
            ),
            tone: 'accent' as const,
            active: true,
          },
        ];
      })
    );

    const outputCables: Cable[] = buses.flatMap((busEntry, busIndex) =>
      busEntry.outputs.flatMap((deviceId, portIndex) => {
        const target = resolveDestination(deviceId, outputIds);
        if (!target) {
          return [];
        }

        const landing =
          target.kind === 'output'
            ? {
                y: outputTop(target.index, destinationFocus) + OUTPUT_PORT_OFFSET,
                tone: outputs[target.index].role === 'CUE' ? ('warn' as const) : ('accent' as const),
              }
            : target.kind === 'cast'
              ? { y: destination.top + CAST_PORT_OFFSET, tone: 'hot' as const }
              : { y: tapeTop(destinationFocus) + TAPE_PORT_OFFSET, tone: 'hot' as const };

        return [
          {
            id: `out-${busEntry.id}-${deviceId}`,
            path: cablePath(busPort(busIndex, busExpansions, portIndex, busEntry.outputs.length, 'out'), {
              x: destination.x,
              y: landing.y,
            }),
            tone: landing.tone,
            active: busEntry.inputs.length > 0,
          },
        ];
      })
    );

    return [...sourceCables, ...outputCables];
  }, [buses, busExpansions, channelDevices, layouts, destinationFocus, outputs, outputIds]);

  return (
    <Box
      // Nodes stop their own clicks, so anything arriving here landed on bare
      // canvas and means "close whatever is open".
      onClick={clearSelection}
      style={{
        position: 'relative',
        width: canvas.width,
        height,
        transformOrigin: 'top left',
        backgroundImage: `radial-gradient(${color.canvasDot} 1px, transparent 1px)`,
        backgroundSize: `${canvas.dotGridSize}px ${canvas.dotGridSize}px`,
      }}
    >
      <CableLayer cables={cables} width={canvas.width} height={height} />

      <Box style={{ position: 'absolute', left: source.x, top: 4 }}>
        <SectionLabel tracking="widest">SOURCES</SectionLabel>
      </Box>
      <Box style={{ position: 'absolute', left: bus.x, top: 4 }}>
        <SectionLabel tracking="widest">MIX BUS</SectionLabel>
      </Box>
      <Box style={{ position: 'absolute', left: destination.x, top: 4 }}>
        <SectionLabel tracking="widest">DESTINATIONS</SectionLabel>
      </Box>

      {channels.map((channel, index) => (
        <ChannelNode
          key={channel.id}
          channel={channel}
          index={index}
          top={channelTop(index, layouts)}
          expansion={layouts[index].expansion}
          variant={layouts[index].variant}
        />
      ))}

      <DashedTarget
        label="+ PATCH A SOURCE"
        hint="device · application · virtual input"
        onClick={() => void addChannel()}
        height={source.addNodeHeight}
        style={{
          position: 'absolute',
          left: source.x,
          top: source.top + sourceStackHeight(layouts),
          width: source.addNodeWidth,
        }}
      />

      {buses.length === 0 ? (
        <Box style={{ position: 'absolute', left: bus.x, top: bus.top, width: bus.width }}>
          <Text size="xs" c={color.textFaint} ta="center">
            No mixes yet. Route a source to a destination and one appears here.
          </Text>
        </Box>
      ) : (
        buses.map((busEntry, busIndex) => (
          <BusNode
            key={busEntry.id}
            bus={busEntry}
            top={busTop(busIndex, busExpansions)}
            expanded={busExpansions[busIndex]}
            onGainChange={(busId, gainDb) => void setBusGain(busId, gainDb)}
            ports={
              <>
                {busEntry.inputs.map((deviceId, portIndex) => (
                  <PortDot
                    key={`in-${deviceId}`}
                    tone="accent"
                    side="left"
                    top={
                      busPort(busIndex, busExpansions, portIndex, busEntry.inputs.length, 'in').y -
                      busTop(busIndex, busExpansions)
                    }
                  />
                ))}
                {busEntry.outputs.map((deviceId, portIndex) => (
                  <PortDot
                    key={`out-${deviceId}`}
                    tone={busEntry.inputs.length > 0 ? 'accent' : 'dead'}
                    side="right"
                    top={
                      busPort(busIndex, busExpansions, portIndex, busEntry.outputs.length, 'out').y -
                      busTop(busIndex, busExpansions)
                    }
                  />
                ))}
              </>
            }
          />
        ))
      )}

      <CastDestination focused={destinationFocus === 'cast'} />
      <TapeDestination top={tapeTop(destinationFocus)} focused={destinationFocus === 'tape'} />

      {outputs.map((output, index) => (
        <OutputDestination
          key={output.id}
          output={output}
          top={outputTop(index, destinationFocus)}
          // Counts the hardware outputs only. The stream and the tape sit above
          // these but carry no number, so counting them would start this at 03.
          position={index}
          options={optionsFor(output.id)}
          switchError={outputErrors[output.id] ?? null}
          onSelect={selectOutput}
          onChangeDevice={handleChangeOutput}
          onCycleRole={cycleOutputRole}
          onGainChange={setOutputGain}
          onRemove={(deviceId) => void removeOutput(deviceId)}
        />
      ))}

      <AddDestination
        top={extraTop(outputs.length, destinationFocus)}
        available={available}
        onPick={selectOutput}
      />
    </Box>
  );
};
