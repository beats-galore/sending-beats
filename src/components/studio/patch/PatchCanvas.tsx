import { Box, Text } from '@mantine/core';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { useChannelsData } from '../../../hooks';
import type { PatchTargetKey } from '../../../services/patch-color-service';
import { useMixerStore } from '../../../stores';
import { orderedBuses, useBusStore } from '../../../stores/bus-store';
import { usePatchColorStore } from '../../../stores/patch-color-store';
import { usePatchLayoutStore } from '../../../stores/patch-layout-store';
import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { useChannelCardVariants } from '../hooks/use-channel-card-variants';
import { useChannelDevices } from '../hooks/use-channel-devices';
import { useFocusedNode } from '../hooks/use-focused-node';
import { patchColorOf } from '../hooks/use-patch-color';
import { usePatchOutputs } from '../hooks/use-patch-outputs';
import { DashedTarget } from '../primitives/DashedTarget';
import { SectionLabel } from '../primitives/SectionLabel';
import { AddDestination } from './AddDestination';
import { BusNode } from './BusNode';
import { CableLayer } from './CableLayer';
import { CastDestination } from './CastDestination';
import { ChannelNode } from './ChannelNode';
import { OutputDestination } from './OutputDestination';
import { patchCables } from './patch-cables';
import { resolvePatchRects } from './patch-rects';
import { PatchRectsProvider } from './patch-rects-context';
import { PinIndicator } from './PinIndicator';
import { PinSeams } from './PinSeams';
import { TapeDestination } from './TapeDestination';

const { source, bus, destination, canvas } = layout;

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

  // Colours, routing and the arrangement all belong to a configuration, so
  // switching patches has to fetch the new one's rather than leave the previous
  // patch's on screen.
  const activeConfigurationId = useMixerStore((state) => state.activeSession?.configuration.id);
  const loadPatchColors = usePatchColorStore((state) => state.load);
  const loadBuses = useBusStore((state) => state.load);
  const loadPatchLayout = usePatchLayoutStore((state) => state.load);
  useEffect(() => {
    void loadPatchColors();
    void loadBuses();
    void loadPatchLayout();
  }, [loadPatchColors, loadBuses, loadPatchLayout, activeConfigurationId]);

  const storedBuses = useBusStore((state) => state.buses);
  const buses = useMemo(() => orderedBuses(storedBuses), [storedBuses]);
  const setBusGain = useBusStore((state) => state.setGain);
  const channelDevices = useChannelDevices();

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

  // Selection is a highlight and the target for the keyboard shortcuts. What a
  // node is showing comes from how big it has been made, not from this.
  const clearSelection = useStudioStore((state) => state.clearSelection);
  const focused = useFocusedNode();
  const selectedId = focused?.kind === 'channel' ? focused.channelId : null;

  const variants = useChannelCardVariants();

  // Memoised because the cables are drawn as animating SVG paths: rebuilding
  // them on every render of the canvas would restart the marching dashes.
  const placements = usePatchLayoutStore((state) => state.placements);
  const rects = useMemo(
    () =>
      resolvePatchRects(
        {
          channels: channels.map((channel) => ({
            id: channel.id,
            variant: variants[channel.id] ?? 'device',
            // Decides how far "open" goes for this source: with its effects
            // switched off there is no chain below the inspector to make room
            // for, so the group opens it only that far.
            effectsEnabled: channel.effects_enabled,
          })),
          busIds: buses.map((busEntry) => busEntry.id),
          outputIds: outputs.map((output) => output.id),
        },
        placements
      ),
    [channels, variants, buses, outputs, placements]
  );

  // Cables are painted in the colour of whatever they carry, the same rule the
  // routing tiles follow, so a patch can be traced by following one colour.
  const patchColors = usePatchColorStore((state) => state.colors);
  const colorFor = useCallback(
    (targetKey: PatchTargetKey, position: number) =>
      patchColorOf(patchColors, targetKey, position).value,
    [patchColors]
  );

  const cables = useMemo(
    () => patchCables({ buses, channelDevices, outputs, rects, colorFor }),
    [buses, channelDevices, outputs, rects, colorFor]
  );

  return (
    // A dragged node has to hit-test itself against every other to know whether
    // it is being dropped against an edge, and a node knows only its own box.
    <PatchRectsProvider value={rects}>
    <Box
      // Nodes stop their own clicks, so anything arriving here landed on bare
      // canvas and means "close whatever is open".
      onClick={clearSelection}
      style={{
        position: 'relative',
        width: canvas.width,
        height: rects.height,
        transformOrigin: 'top left',
        backgroundImage: `radial-gradient(${color.canvasDot} 1px, transparent 1px)`,
        backgroundSize: `${canvas.dotGridSize}px ${canvas.dotGridSize}px`,
      }}
    >
      <CableLayer cables={cables} width={canvas.width} height={rects.height} />

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
          rect={rects.channels[index]}
          variant={variants[channel.id] ?? 'device'}
          selected={channel.id === selectedId}
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
          top: rects.addSourceTop,
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
            rect={rects.buses[busIndex]}
            selected={focused?.kind === 'bus' && focused.busId === busEntry.id}
            onGainChange={(busId, gainDb) => void setBusGain(busId, gainDb)}
          />
        ))
      )}

      <CastDestination rect={rects.cast} selected={focused?.kind === 'cast'} />
      <TapeDestination rect={rects.tape} selected={focused?.kind === 'tape'} />

      {outputs.map((output, index) => (
        <OutputDestination
          key={output.id}
          output={output}
          rect={rects.outputs[index]}
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
        top={rects.addDestinationTop}
        available={available}
        onPick={selectOutput}
      />

      <PinSeams rects={rects} />
      <PinIndicator rects={rects} />
    </Box>
    </PatchRectsProvider>
  );
};
