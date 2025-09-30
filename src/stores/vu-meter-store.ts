import { create } from 'zustand';

type VUMeterStore = {
  channelLevels: Record<number, [number, number, number, number]>;
  masterLevels: {
    left: { peak_level: number; rms_level: number };
    right: { peak_level: number; rms_level: number };
  };
  updateChannelLevels: (levels: Record<number, [number, number, number, number]>) => void;
  updateMasterLevels: (levels: {
    left: { peak_level: number; rms_level: number };
    right: { peak_level: number; rms_level: number };
  }) => void;
  batchUpdate: (updates: {
    channelLevels?: Record<number, [number, number, number, number]>;
    masterLevels?: {
      left: { peak_level: number; rms_level: number };
      right: { peak_level: number; rms_level: number };
    };
  }) => void;
};

export const useVUMeterStore = create<VUMeterStore>((set) => ({
  channelLevels: {},
  masterLevels: {
    left: { peak_level: 0, rms_level: 0 },
    right: { peak_level: 0, rms_level: 0 },
  },

  updateChannelLevels: (levels) => set({ channelLevels: levels }),

  updateMasterLevels: (levels) => set({ masterLevels: levels }),

  batchUpdate: (updates) => {
    set((state) => {
      const newState: Partial<VUMeterStore> = {};

      if (updates.channelLevels) {
        newState.channelLevels = { ...state.channelLevels, ...updates.channelLevels };
      }

      if (updates.masterLevels) {
        newState.masterLevels = updates.masterLevels;
      }

      return Object.keys(newState).length > 0 ? newState : {};
    });
  },
}));