import { useCallback, useMemo } from 'react';

import { useAudioDevices, useMasterSectionData } from '../../../hooks';
import { useConfigurationStore, useMixerStore } from '../../../stores/mixer-store';
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
  /**
   * Why this destination cannot accept the master sum, or null when it can.
   *
   * `live` is read from the saved configuration, which still names a device that
   * failed to connect — an unplugged output would otherwise render as though it
   * were receiving audio.
   */
  unavailableReason: string | null;
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
  const restoreFailures = useMixerStore((state) => state.deviceRestoreFailures);

  const outputRoles = useStudioStore((state) => state.outputRoles);
  const outputGains = useStudioStore((state) => state.outputGains);
  const cycleOutputRole = useStudioStore((state) => state.cycleOutputRole);
  const setOutputGain = useStudioStore((state) => state.setOutputGain);

  const outputs = useMemo<PatchOutput[]>(() => {
    const configured = activeSession?.configuredDevices.filter((device) => !device.isInput) ?? [];

    return configured.map((device) => {
      const restoreFailure = restoreFailures.find(
        (failure) => failure.deviceIdentifier === device.deviceIdentifier
      );
      const present = outputDevices.some((candidate) => candidate.id === device.deviceIdentifier);

      return {
        id: device.deviceIdentifier,
        name: device.deviceName ?? device.deviceIdentifier,
        live: mixerConfig?.master_output_device_id === device.deviceIdentifier,
        role: outputRoles[device.deviceIdentifier] ?? 'MAIN',
        gainDb: outputGains[device.deviceIdentifier] ?? 0,
        unavailableReason:
          restoreFailure?.reason ?? (present ? null : 'Device is not currently available'),
      };
    });
  }, [
    activeSession,
    mixerConfig?.master_output_device_id,
    outputRoles,
    outputGains,
    restoreFailures,
    outputDevices,
  ]);

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
