import { Box } from '@mantine/core';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { useChannelsData } from '../../../hooks';
import { useMixerStore } from '../../../stores';
import { usePatchColorStore } from '../../../stores/patch-color-store';
import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { useChannelCardVariants } from '../hooks/use-channel-card-variants';
import { useFocusedNode } from '../hooks/use-focused-node';
import { usePatchOutputs } from '../hooks/use-patch-outputs';
import { useStreamTransport } from '../hooks/use-stream-transport';
import { useTapeTransport } from '../hooks/use-tape-transport';
import { DashedTarget } from '../primitives/DashedTarget';
import { PortDot } from '../primitives/PortDot';
import { SectionLabel } from '../primitives/SectionLabel';
import { AddDestination } from './AddDestination';
import { CableLayer } from './CableLayer';
import type { Cable } from './CableLayer';
import { CastDestination } from './CastDestination';
import { ChannelNode } from './ChannelNode';
import { MasterBus } from './MasterBus';
import { OutputDestination } from './OutputDestination';
import {
  busInPort,
  busOutPort,
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
  const { isLive } = useStreamTransport();
  const tape = useTapeTransport();

  // Colours belong to a configuration, so switching patches has to fetch the
  // new one's rather than leave the previous patch's colours on screen.
  const activeConfigurationId = useMixerStore((state) => state.activeSession?.configuration.id);
  const loadPatchColors = usePatchColorStore((state) => state.load);
  useEffect(() => {
    void loadPatchColors();
  }, [loadPatchColors, activeConfigurationId]);

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

  const destinationCount = 2 + outputs.length;
  const height = canvasHeight(layouts, destinationFocus, outputs.length, 0, false);

  const cables = useMemo<Cable[]>(() => {
    const sourceCables: Cable[] = channels.map((channel, index) => ({
      id: `ch-${channel.id}`,
      path: cablePath(channelPort(index, layouts), busInPort(index, channels.length)),
      tone: 'accent',
      active: true,
    }));

    const castCable: Cable = {
      id: 'cast',
      path: cablePath(busOutPort(0, destinationCount), {
        x: destination.x,
        y: destination.top + CAST_PORT_OFFSET,
      }),
      tone: 'hot',
      active: isLive,
    };

    const tapeCable: Cable = {
      id: 'tape',
      path: cablePath(busOutPort(1, destinationCount), {
        x: destination.x,
        y: tapeTop(destinationFocus) + TAPE_PORT_OFFSET,
      }),
      tone: 'hot',
      active: tape.isRecording,
    };

    const outputCables: Cable[] = outputs.map((output, index) => ({
      id: `out-${output.id}`,
      path: cablePath(busOutPort(2 + index, destinationCount), {
        x: destination.x,
        y: outputTop(index, destinationFocus) + OUTPUT_PORT_OFFSET,
      }),
      tone: output.role === 'CUE' ? 'warn' : 'accent',
      active: output.live,
    }));

    return [...sourceCables, castCable, tapeCable, ...outputCables];
  }, [
    channels,
    layouts,
    destinationFocus,
    destinationCount,
    isLive,
    tape.isRecording,
    outputs,
  ]);

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

      <MasterBus
        inputCount={channels.length}
        outputCount={destinationCount}
        inPorts={channels.map((channel, index) => (
          <PortDot
            key={channel.id}
            tone="accent"
            side="left"
            top={busInPort(index, channels.length).y - bus.top}
          />
        ))}
        outPorts={
          <>
            <PortDot
              tone={isLive ? 'hot' : 'dead'}
              side="right"
              top={busOutPort(0, destinationCount).y - bus.top}
            />
            <PortDot
              tone={tape.isRecording ? 'hot' : 'dead'}
              side="right"
              top={busOutPort(1, destinationCount).y - bus.top}
            />
            {outputs.map((output, index) => (
              <PortDot
                key={output.id}
                tone={output.live ? (output.role === 'CUE' ? 'warn' : 'accent') : 'dead'}
                side="right"
                top={busOutPort(2 + index, destinationCount).y - bus.top}
              />
            ))}
          </>
        }
      />

      <CastDestination focused={destinationFocus === 'cast'} />
      <TapeDestination top={tapeTop(destinationFocus)} focused={destinationFocus === 'tape'} />

      {outputs.map((output, index) => (
        <OutputDestination
          key={output.id}
          output={output}
          top={outputTop(index, destinationFocus)}
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
