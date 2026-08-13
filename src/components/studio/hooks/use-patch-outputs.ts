import { useCallback, useMemo } from 'react';

import { useAudioDevices, useMasterSectionData } from '../../../hooks';
import { useConfigurationStore } from '../../../stores/mixer-store';
import { useStudioStore } from '../../../stores/studio-store';
import type { DestinationRole } from '../../../stores/studio-store';
import { asDeviceIdentifier } from '../../../types/device-identifier';

export type PatchOutput = {
  id: string;
  name: string;
  /** This device is the one the master sum is currently feeding. */
  live: boolean;
  role: DestinationRole;
  gainDb: number;
};

/**
 * The destinations the master sum feeds, and the devices it could feed.
 *
 * The engine drives one master output at a time, so exactly one entry is live;
 * selecting another switches the stream over. Role and trim are interface state
 * (see `studio-store`) until the pipeline can carry more than one output.
 */
export const usePatchOutputs = () => {
  const { activeSession } = useConfigurationStore();
  const { mixerConfig, setMasterOutputDevice } = useMasterSectionData();
  const { outputDevices } = useAudioDevices();

  const outputRoles = useStudioStore((state) => state.outputRoles);
  const outputGains = useStudioStore((state) => state.outputGains);
  const cycleOutputRole = useStudioStore((state) => state.cycleOutputRole);
  const setOutputGain = useStudioStore((state) => state.setOutputGain);

  const outputs = useMemo<PatchOutput[]>(() => {
    const configured = activeSession?.configuredDevices.filter((device) => !device.isInput) ?? [];

    return configured.map((device) => ({
      id: device.deviceIdentifier,
      name: device.deviceName ?? device.deviceIdentifier,
      live: mixerConfig?.master_output_device_id === device.deviceIdentifier,
      role: outputRoles[device.deviceIdentifier] ?? 'MAIN',
      gainDb: outputGains[device.deviceIdentifier] ?? 0,
    }));
  }, [activeSession, mixerConfig?.master_output_device_id, outputRoles, outputGains]);

  const available = useMemo(
    () => outputDevices.filter((device) => !outputs.some((output) => output.id === device.id)),
    [outputDevices, outputs]
  );

  const selectOutput = useCallback(
    (deviceId: string) => {
      setMasterOutputDevice(asDeviceIdentifier(deviceId));
    },
    [setMasterOutputDevice]
  );

  return { outputs, available, selectOutput, cycleOutputRole, setOutputGain };
};
