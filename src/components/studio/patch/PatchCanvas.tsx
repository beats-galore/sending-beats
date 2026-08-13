import { Box } from '@mantine/core';
import { useMemo } from 'react';

import { useChannelsData } from '../../../hooks';
import { useMixerStore } from '../../../stores';
import { useStudioStore } from '../../../stores/studio-store';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
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
} from './patch-geometry';
import { TapeDestination } from './TapeDestination';

const { source, bus, destination, canvas } = layout;

const CAST_PORT_OFFSET = 83;
const TAPE_PORT_OFFSET = 63;
const OUTPUT_PORT_OFFSET = 24;

/** The wiring diagram: sources on the left, the sum in the middle, destinations on the right. */
export const PatchCanvas = () => {
  const { channels } = useChannelsData();
  const addChannel = useMixerStore((state) => state.addChannel);
  const selectedChannelId = useStudioStore((state) => state.selectedChannelId);
  const { outputs, available, selectOutput, cycleOutputRole, setOutputGain } = usePatchOutputs();
  const { isLive } = useStreamTransport();
  const tape = useTapeTransport();

  const selectedId = selectedChannelId ?? (channels.length > 0 ? channels[0].id : null);
  const expanded = channels.map((channel) => channel.id === selectedId);

  const destinationCount = 2 + outputs.length;
  const height = canvasHeight(expanded, outputs.length, 0, false);

  const cables = useMemo<Cable[]>(() => {
    const sourceCables: Cable[] = channels.map((channel, index) => ({
      id: `ch-${channel.id}`,
      path: cablePath(channelPort(index, expanded), busInPort(index, channels.length)),
      tone: 'accent',
      active: true,
    }));

    const castCable: Cable = {
      id: 'cast',
      path: cablePath(busOutPort(0, destinationCount), {
        x: destination.x,
        y: destination.castTop + CAST_PORT_OFFSET,
      }),
      tone: 'hot',
      active: isLive,
    };

    const tapeCable: Cable = {
      id: 'tape',
      path: cablePath(busOutPort(1, destinationCount), {
        x: destination.x,
        y: destination.tapeTop + TAPE_PORT_OFFSET,
      }),
      tone: 'hot',
      active: tape.isRecording,
    };

    const outputCables: Cable[] = outputs.map((output, index) => ({
      id: `out-${output.id}`,
      path: cablePath(busOutPort(2 + index, destinationCount), {
        x: destination.x,
        y: outputTop(index) + OUTPUT_PORT_OFFSET,
      }),
      tone: output.role === 'CUE' ? 'warn' : 'accent',
      active: output.live,
    }));

    return [...sourceCables, castCable, tapeCable, ...outputCables];
  }, [channels, expanded, destinationCount, isLive, tape.isRecording, outputs]);

  return (
    <Box
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
          top={channelTop(index, expanded)}
          expanded={expanded[index]}
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
          top: source.top + sourceStackHeight(expanded),
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

      <CastDestination />
      <TapeDestination />

      {outputs.map((output, index) => (
        <OutputDestination
          key={output.id}
          output={output}
          top={outputTop(index)}
          onSelect={selectOutput}
          onCycleRole={cycleOutputRole}
          onGainChange={setOutputGain}
        />
      ))}

      <AddDestination top={extraTop(outputs.length)} available={available} onPick={selectOutput} />
    </Box>
  );
};
