import { useCastConfigurationStore } from '../../../stores/cast-configuration-store';
import { castOutputId } from '../patch/destination-target';

export type CastDestinationTarget = {
  /** The station's own id */
  castConfigurationId: string;
  /** What the mixing layer routes the broadcast by */
  deviceId: string;
  name: string;
};

/**
 * The broadcast as somewhere sources can be routed to, or null when this patch
 * has no station on it.
 *
 * A cast is on the canvas because it was put there, the same as any other
 * destination — offering one that has not been added would be a tile with
 * nothing behind it.
 *
 * The identifier comes from the station's row rather than the running stream, so
 * routing can be set while off air and still be pointing at the same output when
 * it goes live.
 */
export const useCastDestination = (): CastDestinationTarget | null => {
  const configurations = useCastConfigurationStore((state) => state.configurations);
  const targetIds = useCastConfigurationStore((state) => state.targetIds);

  const station = configurations.find((entry) => targetIds.includes(entry.id));

  if (!station) {
    return null;
  }

  return {
    castConfigurationId: station.id,
    deviceId: castOutputId(station.id),
    name: station.name,
  };
};

/** Stations that could be added to this patch, because they are not on it yet. */
export const useAvailableCastTargets = () => {
  const configurations = useCastConfigurationStore((state) => state.configurations);
  const targetIds = useCastConfigurationStore((state) => state.targetIds);

  return configurations.filter((entry) => !targetIds.includes(entry.id));
};
