import { useCallback, useMemo } from 'react';

import { useAudioDevices, useMasterSectionData } from '../../../hooks';
import { audioService } from '../../../services';
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
  const { activeSession, removeConfiguredDevice } = useConfigurationStore();
  const { mixerConfig, setMasterOutputDevice } = useMasterSectionData();
  const { outputDevices, disconnectedDeviceIds } = useAudioDevices();
  const restoreFailures = useMixerStore((state) => state.deviceRestoreFailures);
  const changeOutputDevice = useMixerStore((state) => state.changeOutputDevice);

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
        // The watcher clears this the moment a device reappears and only puts
        // it back when rebuilding the stream failed, so it outranks presence:
        // the hardware is there but the master sum is not reaching it.
        //
        // Presence in turn wins over the restore record, which is only a
        // snapshot of what failed at startup. A device plugged back in
        // afterwards is available again, and reporting it offline hides the
        // live destination.
        unavailableReason: disconnectedDeviceIds.includes(device.deviceIdentifier)
          ? 'Device reconnected but its stream could not be restored'
          : present
            ? null
            : (restoreFailure?.reason ?? 'Device is not currently available'),
      };
    });
  }, [
    activeSession,
    mixerConfig?.master_output_device_id,
    outputRoles,
    outputGains,
    restoreFailures,
    disconnectedDeviceIds,
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

  /**
   * Every device a destination could be pointed at.
   *
   * Devices patched into *other* destinations are left out, since the pipeline
   * registers each output once, but the destination's own device stays so the
   * select has something to show as selected.
   */
  const optionsFor = useCallback(
    (deviceId: string) => {
      const takenElsewhere = new Set(
        outputs.filter((output) => output.id !== deviceId).map((output) => output.id)
      );

      const choices = outputDevices
        .filter((device) => !takenElsewhere.has(device.id))
        .map((device) => ({ value: device.id, label: device.name }));

      // A destination pointed at a device that has gone away still needs an
      // entry, or the select would show a different device as patched.
      if (!choices.some((choice) => choice.value === deviceId)) {
        const output = outputs.find((candidate) => candidate.id === deviceId);
        choices.unshift({
          value: deviceId,
          label: `${output?.name ?? deviceId} (unavailable)`,
        });
      }

      return choices;
    },
    [outputDevices, outputs]
  );

  const changeOutput = useCallback(
    async (oldDeviceId: string, newDeviceId: string) =>
      changeOutputDevice(asDeviceIdentifier(oldDeviceId), asDeviceIdentifier(newDeviceId)),
    [changeOutputDevice]
  );

  const removeOutput = useCallback(
    async (deviceId: string) => {
      try {
        await audioService.removeOutputStream(asDeviceIdentifier(deviceId));
      } catch (error) {
        console.error(`Failed to remove output ${deviceId}:`, error);
        return;
      }
      removeConfiguredDevice(deviceId);
    },
    [removeConfiguredDevice]
  );

  return {
    outputs,
    available,
    optionsFor,
    selectOutput,
    changeOutput,
    removeOutput,
    cycleOutputRole,
    setOutputGain,
  };
};
