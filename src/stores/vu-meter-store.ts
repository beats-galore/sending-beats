import { create } from 'zustand';

/** Peak and RMS for one side, already converted to the 0-1 range meters draw. */
type SideLevels = { peak_level: number; rms_level: number };

export type StereoLevels = {
  left: SideLevels;
  right: SideLevels;
};

const SILENT: StereoLevels = {
  left: { peak_level: 0, rms_level: 0 },
  right: { peak_level: 0, rms_level: 0 },
};

type VUMeterStore = {
  channelLevels: Record<number, [number, number, number, number]>;
  /** Levels per bus, measured after that bus's own gain. Keyed by bus id. */
  busLevels: Record<string, StereoLevels>;
  masterLevels: StereoLevels;
  updateChannelLevels: (levels: Record<number, [number, number, number, number]>) => void;
  updateMasterLevels: (levels: StereoLevels) => void;
  batchUpdate: (updates: {
    channelLevels?: Record<number, [number, number, number, number]>;
    busLevels?: Record<string, StereoLevels>;
    masterLevels?: StereoLevels;
  }) => void;
};

export const useVUMeterStore = create<VUMeterStore>((set) => ({
  channelLevels: {},
  busLevels: {},
  masterLevels: SILENT,

  updateChannelLevels: (levels) => set({ channelLevels: levels }),

  updateMasterLevels: (levels) => set({ masterLevels: levels }),

  batchUpdate: (updates) => {
    set((state) => {
      const newState: Partial<VUMeterStore> = {};

      if (updates.channelLevels) {
        newState.channelLevels = { ...state.channelLevels, ...updates.channelLevels };
      }

      // Merged rather than replaced, the same way channel levels are: a batch
      // carries whichever buses reported in it, not the full set.
      if (updates.busLevels) {
        newState.busLevels = { ...state.busLevels, ...updates.busLevels };
      }

      if (updates.masterLevels) {
        newState.masterLevels = updates.masterLevels;
      }

      return Object.keys(newState).length > 0 ? newState : {};
    });
  },
}));

/** A bus that has not reported yet reads as silent rather than as missing. */
export const useBusLevels = (busId: string): StereoLevels =>
  useVUMeterStore((state) => state.busLevels[busId] ?? SILENT);
