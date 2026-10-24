import { useIsMutating, useMutation } from "@tanstack/react-query";
import { useEffect } from "react";
import { create } from "zustand";

import { type BatchProgress, type BatchRow, commands, events } from "@/lib/bindings.gen";
import { jobKey } from "@/lib/query";
import { runSettings, STANDARD } from "@/modules/repair";
import { DEFAULT_PACKAGE_FORMAT, useSettings } from "@/modules/settings";
import { unwrap } from "@/utils";

/** The scan, a mutation of the run job's (`jobKey`): the backend runs one job at a time. */
const scanKey = [...jobKey, "scan"] as const;

/** The archive of the scan under way that is running. */
export interface ScanAt {
  index: number;
  total: number;
  archive: string;
}

interface ScanState {
  /** The rows of the scans so far, by archive path: a scan adds to what earlier ones found. */
  rows: Record<string, BatchRow>;
  /** The archives of the scan under way, by index. */
  archives: string[];
  /** The archive running, while a scan is under way. */
  at: ScanAt | null;
  /** Whether the person asked the scan to stop. */
  stopping: boolean;
  start: (archives: string[]) => void;
  heard: (progress: BatchProgress) => void;
  end: () => void;
  setStopping: () => void;
  forget: () => void;
}

/** The scan's state, kept while the person moves between pages (#ADR-0107). */
export const useScanStore = create<ScanState>()((set) => ({
  rows: {},
  archives: [],
  at: null,
  stopping: false,
  start: (archives) => set({ archives, at: null, stopping: false }),
  heard: ({ index, total, archive, row }) =>
    set((state) => {
      if (!row) {
        return { at: { index, total, archive } };
      }
      const path = state.archives[index];
      return path === undefined ? {} : { rows: { ...state.rows, [path]: row } };
    }),
  end: () => set({ at: null, stopping: false, archives: [] }),
  setStopping: () => set({ stopping: true }),
  forget: () => set({ rows: {} }),
}));

/** Whether a scan is under way. */
export function useScanRunning(): boolean {
  return useIsMutating({ mutationKey: scanKey }) > 0;
}

/**
 * Scans archives: a batch of ports through `decide`, which writes nothing and keeps no run,
 * its rows heard as each archive ends.
 */
export function useScan() {
  const start = useScanStore((state) => state.start);
  const end = useScanStore((state) => state.end);
  const format = useSettings().data?.["output.format"] ?? DEFAULT_PACKAGE_FORMAT;
  return useMutation({
    mutationKey: scanKey,
    mutationFn: async (archives: string[]) => {
      start(archives);
      return unwrap(
        await commands.runBatch({
          kind: "port",
          through: "decide",
          archives,
          settings: runSettings(STANDARD),
          out: null,
          format,
        }),
      );
    },
    onSettled: () => end(),
  });
}

/** Stops the scan: the archive under way ends after its step, and no archive after it runs. */
export function useStopScan() {
  const setStopping = useScanStore((state) => state.setStopping);
  return useMutation({
    mutationFn: async () => {
      setStopping();
      unwrap(await commands.cancelRun());
    },
  });
}

/** Follows the scan under way through the batch's events, into the store. */
export function useScanEvents() {
  const heard = useScanStore((state) => state.heard);
  useEffect(() => {
    const stop = events.batchProgress.listen(({ payload }) => heard(payload));
    return () => {
      void stop.then((unlisten) => unlisten());
    };
  }, [heard]);
}
