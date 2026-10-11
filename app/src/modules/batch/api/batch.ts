import { useIsMutating, useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { m } from "@/i18n";
import { type BatchRequest, commands, events } from "@/lib/bindings.gen";
import { jobKey, runsKey } from "@/lib/query";
import { pickFiles, pickFolder } from "@/modules/inputs";
import { ARCHIVE_EXTENSIONS } from "@/modules/repair";
import { unwrap } from "@/utils";

import { useBatchStore } from "../state/batchStore";

/** The batch, a mutation of the run job's (`jobKey`): the backend runs one job at a time. */
const batchKey = [...jobKey, "batch"] as const;

/** Whether a batch is under way. */
export function useBatchRunning(): boolean {
  return useIsMutating({ mutationKey: batchKey }) > 0;
}

/** Runs a batch, its table filled from its events, and asks the store's reads again after. */
export function useRunBatch() {
  const start = useBatchStore((state) => state.start);
  const end = useBatchStore((state) => state.end);
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: batchKey,
    mutationFn: async (request: BatchRequest) => {
      start();
      return unwrap(await commands.runBatch(request));
    },
    onSuccess: (view) => end(view),
    onError: () => end(null),
    onSettled: () => void queryClient.invalidateQueries({ queryKey: runsKey }),
  });
}

/** Stops the batch: the archive under way ends after its step, and no archive after it runs. */
export function useStopBatch() {
  const setStopping = useBatchStore((state) => state.setStopping);
  return useMutation({
    mutationFn: async () => {
      setStopping();
      unwrap(await commands.cancelRun());
    },
  });
}

/** Follows the batch under way through its events, into the page's table. */
export function useBatchEvents() {
  const heard = useBatchStore((state) => state.heard);
  useEffect(() => {
    const stop = events.batchProgress.listen(({ payload }) => heard(payload));
    return () => {
      void stop.then((unlisten) => unlisten());
    };
  }, [heard]);
}

/** Adds the archives of a folder the person picks, not its subfolders'. */
export function useAddFolder() {
  const addArchives = useBatchStore((state) => state.addArchives);
  return useMutation({
    mutationFn: async () => {
      const folder = await pickFolder();
      if (folder) {
        addArchives(unwrap(await commands.listArchives(folder)));
      }
    },
  });
}

/** Adds the archives the person picks. */
export async function addFiles(addArchives: (paths: string[]) => void) {
  addArchives(await pickFiles([...ARCHIVE_EXTENSIONS], m.batch_archive_filter_label()));
}
