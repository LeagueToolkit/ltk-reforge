import { create } from "zustand";

import type { BatchProgress, BatchRow, BatchView, RunKind } from "@/lib/bindings.gen";

import { withArchives } from "../outcomes";

/** How far the batch under way is: how many archives it holds, and which are running. */
export interface BatchAt {
  total: number;
  /** The archives started and not ended, by index (#ADR-0108). */
  running: number[];
}

interface BatchState {
  /** The archives to run, in order. */
  archives: string[];
  /** What every run makes of its mod. */
  kind: RunKind;
  /** Whether the runs take the run page's advanced settings in place of the standard ones. */
  runPageSettings: boolean;
  /** The rows of the batch under way or last ended. */
  rows: BatchRow[];
  /** How far the batch is, while one is under way. */
  at: BatchAt | null;
  /** The last batch as the backend answered it, once it ended. */
  ended: BatchView | null;
  /** Whether the person asked the batch to stop. */
  stopping: boolean;
  addArchives: (paths: string[]) => void;
  removeArchive: (path: string) => void;
  /** Takes the archives off the list, and the table with them. */
  clear: () => void;
  setKind: (kind: RunKind) => void;
  setRunPageSettings: (on: boolean) => void;
  /** Empties the table for a batch starting. */
  start: () => void;
  /** Takes an archive starting or ending, as the batch's events tell it. */
  heard: (progress: BatchProgress) => void;
  /** Takes the batch as it ended: its rows replace the ones heard. */
  end: (view: BatchView | null) => void;
  setStopping: () => void;
}

/** The batch page's state, kept while the user moves between pages. */
export const useBatchStore = create<BatchState>()((set) => ({
  archives: [],
  kind: "port",
  runPageSettings: false,
  rows: [],
  at: null,
  ended: null,
  stopping: false,
  addArchives: (paths) => set((state) => ({ archives: withArchives(state.archives, paths) })),
  removeArchive: (path) =>
    set((state) => ({ archives: state.archives.filter((archive) => archive !== path) })),
  clear: () => set({ archives: [], rows: [], ended: null }),
  setKind: (kind) => set({ kind }),
  setRunPageSettings: (runPageSettings) => set({ runPageSettings }),
  start: () => set({ rows: [], at: null, ended: null, stopping: false }),
  heard: ({ index, total, row }) =>
    set((state) => {
      const running = state.at?.running ?? [];
      if (row) {
        return {
          rows: [...state.rows.filter((seen) => seen.index !== index), row],
          at: { total, running: running.filter((seen) => seen !== index) },
        };
      }
      return { at: { total, running: [...running, index] } };
    }),
  end: (view) =>
    set((state) => ({ rows: view?.rows ?? state.rows, ended: view, at: null, stopping: false })),
  setStopping: () => set({ stopping: true }),
}));
