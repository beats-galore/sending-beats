import { useCallback, useMemo } from 'react';

import { useAudioDevices, useMasterSectionData } from '../../../hooks';
import { audioService } from '../../../services';
import { sourcesOf, useBusStore } from '../../../stores/bus-store';
import { useConfigurationStore, useMixerStore } from '../../../stores/mixer-store';
import { useStudioStore } from '../../../stores/studio-store';
import type { DestinationRole } from '../../../stores/studio-store';
import { asDeviceIdentifier } from '../../../types/device-identifier';

export type PatchOutput = {
  id: string;
  name: string;
  /**
   * At least one source reaches this destination.
   *
   * Read from the routing rather than from `master_output_device_id`, which
   * names the single device the engine drove before buses existed. Every
   * destination but that one read as idle under the old test, so a routed
   * output rendered greyed out while it was carrying audio.
   */
  live: boolean;
  role: DestinationRole;
  gainDb: number;
  /**
   * Why this destination cannot accept audio, or null when it can.
   *
   * Kept apart from `live`: a destination can be routed and still be offline,
   * and an unplugged output would otherwise render as though it were receiving.
   */
  unavailableReason: string | null;
};

/**
 * The destinations audio reaches, and the devices it could reach.
 *
 * Role and trim are interface state (see `studio-store`) until the pipeline can
 * carry them.
 */
export const usePatchOutputs = () => {
  const { activeSession, removeConfiguredDevice } = useConfigurationStore();
  const { setMasterOutputDevice } = useMasterSectionData();
  const { outputDevices, disconnectedDeviceIds } = useAudioDevices();
  const restoreFailures = useMixerStore((state) => state.deviceRestoreFailures);
  const changeOutputDevice = useMixerStore((state) => state.changeOutputDevice);

  const buses = useBusStore((state) => state.buses);
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
        live: sourcesOf(buses, device.deviceIdentifier).length > 0,
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
    buses,
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
