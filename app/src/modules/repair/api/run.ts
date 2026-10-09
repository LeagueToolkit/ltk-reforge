import { queryOptions, useIsMutating, useMutation, useQuery } from "@tanstack/react-query";
import { useEffect } from "react";

import {
  commands,
  events,
  type RunKind,
  type RunRequest,
  type StageName,
} from "@/lib/bindings.gen";
import { queryFn, unwrap } from "@/utils";

import { startedRun, useRepairStore, withPart } from "../state/repairStore";

/** The stages of a run of the kind, as the library names them. They are built into the app. */
export const runStagesQuery = (kind: RunKind) =>
  queryOptions({
    queryKey: ["run", "stages", kind] as const,
    queryFn: queryFn(() => commands.runStages(kind)),
    staleTime: Infinity,
  });

/** The stages of a run of the kind, in order. None while the inputs do not open. */
export function useRunStages(kind: RunKind): StageName[] {
  return useQuery(runStagesQuery(kind)).data ?? [];
}

/**
 * The run of a mod, a mutation under this key. The backend runs one at a time, and the two
 * views share it: the one switched to mid-run shows the run under way.
 */
const runKey = ["run", "mod"] as const;

/** Whether a run is under way, from either view. */
export function useRunRunning(): boolean {
  return useIsMutating({ mutationKey: runKey }) > 0;
}

/** Runs a mod as a request asks, and keeps its answer as the page's last run. */
export function useRunMod() {
  const setLast = useRepairStore((state) => state.setLast);
  const setLive = useRepairStore((state) => state.setLive);
  return useMutation({
    mutationKey: runKey,
    mutationFn: async (request: RunRequest) => {
      setLive(() => startedRun());
      return unwrap(await commands.runMod(request));
    },
    onSuccess: (view) => setLast(view),
    onSettled: () => setLive(() => null),
  });
}

/** Follows the run under way through its events, into the page's live run. */
export function useRunEvents() {
  const setLive = useRepairStore((state) => state.setLive);
  useEffect(() => {
    const stops = [
      events.runStage.listen(({ payload }) =>
        setLive((live) => {
          if (!live) {
            return live;
          }
          const stage =
            payload.event.event === "started"
              ? { state: "running" as const }
              : { state: "ended" as const, millis: payload.event.millis };
          return { ...live, stages: { ...live.stages, [payload.stage]: stage } };
        }),
      ),
      events.runFinding.listen(({ payload }) =>
        setLive((live) => live && { ...live, findings: [...live.findings, payload.finding] }),
      ),
      events.runLog.listen(({ payload }) =>
        setLive((live) => live && { ...live, log: [...live.log, payload.line] }),
      ),
      events.runPart.listen(({ payload }) =>
        setLive((live) => live && withPart(live, payload.part)),
      ),
    ];
    return () => {
      for (const stop of stops) {
        void stop.then((unlisten) => unlisten());
      }
    };
  }, [setLive]);
}

/** The folder a path is in: the folder a package is written in when none is chosen. */
export function folderOf(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut > 0 ? path.slice(0, cut) : path;
}
