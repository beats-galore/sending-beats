import { create } from 'zustand';

import { busService } from '../services/bus-service';
import type { Bus } from '../types/bus.types';
import { MAIN_BUS_ID } from '../types/bus.types';

type BusStore = {
  buses: Bus[];
  loaded: boolean;

  /** Restore the stored routing and hold what came back */
  load: () => Promise<void>;
  setOutputSources: (deviceId: string, inputIds: string[]) => Promise<void>;
  setGain: (busId: string, gainDb: number) => Promise<void>;
};

export const useBusStore = create<BusStore>((set) => ({
  buses: [],
  loaded: false,

  load: async () => {
    try {
      // Restore rather than list: nothing else calls it, so this is what puts a
      // saved patch's routing back over the devices that have just registered.
      set({ buses: await busService.restore(), loaded: true });
    } catch (error) {
      console.error('Failed to load bus routing:', error);
      set({ loaded: true });
    }
  },

  setOutputSources: async (deviceId, inputIds) => {
    try {
      // Not applied optimistically: the engine decides which bus a destination
      // lands on, and two destinations given the same inputs are merged onto
      // one. Guessing that here would show a split that does not exist.
      set({ buses: await busService.setOutputSources(deviceId, inputIds) });
    } catch (error) {
      console.error('Failed to route destination:', error);
    }
  },

  setGain: async (busId, gainDb) => {
    // Applied locally first, unlike routing: a drag emits continuously and
    // waiting for each round trip would make the control lag the pointer. The
    // engine cannot move a bus in response to a trim, so there is nothing for
    // the local value to disagree with.
    const gain = 10 ** (gainDb / 20);
    set((state) => ({
      buses: state.buses.map((bus) => (bus.id === busId ? { ...bus, gain } : bus)),
    }));

    try {
      await busService.setGain(busId, gain);
    } catch (error) {
      console.error('Failed to set bus gain:', error);
    }
  },
}));

/** The input device identifiers reaching a destination, in whatever bus it is on. */
export const sourcesOf = (buses: Bus[], deviceId: string): string[] =>
  buses.find((bus) => bus.outputs.includes(deviceId))?.inputs ?? [];

/**
 * Buses in the order the canvas draws them, main first.
 *
 * The engine returns them keyed order, where a generated bus can sort ahead of
 * main. Main is the one a device falls back to, so it holds the top of the
 * column and everything routed away from it reads as a departure from there.
 */
export const orderedBuses = (buses: Bus[]): Bus[] =>
  [...buses].sort((a, b) => {
    if (a.id === MAIN_BUS_ID) {
      return -1;
    }
    if (b.id === MAIN_BUS_ID) {
      return 1;
    }
    return a.name.localeCompare(b.name);
  });
