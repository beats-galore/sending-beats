import { useCallback } from 'react';

import { useCastConfigurationStore } from '../../../stores/cast-configuration-store';
import { color } from '../../../theme/tokens';
import type { CastConfiguration, CastConfigurationInput } from '../../../types/cast.types';
import { toInput } from '../../../types/cast.types';

type StationFields = {
  station: CastConfiguration | null;
  /** Change some of a station's fields, leaving the rest as they are */
  edit: (changes: Partial<CastConfigurationInput>) => void;
  /** Spread onto an input to hold it while the station is on air */
  locked: {
    readOnly: boolean;
    styles?: { input: { color: string; cursor: string } };
  };
};

/**
 * A station and the two things every field of it needs.
 *
 * The station is fetched by id rather than passed down, so the per-protocol
 * field sets stay independent of whatever is composing them.
 *
 * Connection fields are read only while on air: the target was handed to the
 * engine when the broadcast started, and editing it afterwards would show
 * settings that have no bearing on what is going out.
 */
export const useStationFields = (stationId: string, isLive: boolean): StationFields => {
  const station = useCastConfigurationStore(
    (state) => state.configurations.find((entry) => entry.id === stationId) ?? null
  );
  const update = useCastConfigurationStore((state) => state.update);

  const edit = useCallback(
    (changes: Partial<CastConfigurationInput>) => {
      if (station) {
        void update(station.id, { ...toInput(station), ...changes });
      }
    },
    [station, update]
  );

  return {
    station,
    edit,
    // Read-only inputs keep their layout but read as settled rather than
    // editable, so a live target does not look like a field waiting for input.
    locked: {
      readOnly: isLive,
      styles: isLive ? { input: { color: color.textDim, cursor: 'default' } } : undefined,
    },
  };
};
