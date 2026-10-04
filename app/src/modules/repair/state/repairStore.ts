import { create } from "zustand";

import type { RunSettings, RunView, StageName } from "@/lib/bindings.gen";

/** Which view of the run the repair page shows (#ADR-0050). */
export type RepairMode = "simple" | "advanced";

/** The settings of the advanced view, as the run request takes them. */
export interface AdvancedSettings {
  through: StageName;
  fixupsOff: string[];
  fixupsOn: string[];
  checksOff: string[];
  keepStale: boolean;
  block: string[];
}

const STANDARD: AdvancedSettings = {
  through: "write",
  fixupsOff: [],
  fixupsOn: [],
  checksOff: [],
  keepStale: false,
  block: [],
};

/** A list with an item added when absent and taken out when present. */
export function toggled(list: string[], item: string): string[] {
  return list.includes(item) ? list.filter((x) => x !== item) : [...list, item].sort();
}

/** The advanced settings as the run request takes them. */
export function runSettings(settings: AdvancedSettings): RunSettings {
  return {
    fixups_off: settings.fixupsOff,
    fixups_on: settings.fixupsOn,
    checks_off: settings.checksOff,
    keep_stale: settings.keepStale,
    block: settings.block,
  };
}

interface RepairState {
  /** The mod archive picked. */
  archive: string | null;
  /** The view shown. */
  mode: RepairMode;
  /** The last run of the archive, as the backend answered it. */
  last: RunView | null;
  /** The advanced view's settings. */
  advanced: AdvancedSettings;
  setArchive: (archive: string | null) => void;
  setMode: (mode: RepairMode) => void;
  setLast: (last: RunView | null) => void;
  setAdvanced: (change: Partial<AdvancedSettings>) => void;
  resetAdvanced: () => void;
}

/** The repair page's state, kept while the user moves between pages. */
export const useRepairStore = create<RepairState>()((set) => ({
  archive: null,
  mode: "simple",
  last: null,
  advanced: STANDARD,
  setArchive: (archive) => set({ archive, last: null }),
  setMode: (mode) => set({ mode }),
  setLast: (last) => set({ last }),
  setAdvanced: (change) => set((state) => ({ advanced: { ...state.advanced, ...change } })),
  resetAdvanced: () => set({ advanced: STANDARD }),
}));
