import {
  queryOptions,
  useIsMutating,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useEffect } from "react";
import { match } from "ts-pattern";

import {
  commands,
  events,
  type RunKind,
  type RunRequest,
  type StageEvent,
  type StageName,
} from "@/lib/bindings.gen";
import { jobKey, runsKey } from "@/lib/query";
import { queryFn, unwrap } from "@/utils";

import { type LiveStage, startedRun, useRepairStore, withPart } from "../state/repairStore";

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
const runKey = [...jobKey, "mod"] as const;

/** Whether a run is under way, from either view. */
export function useRunRunning(): boolean {
  return useIsMutating({ mutationKey: runKey }) > 0;
}

/** Whether the run job is busy: a run of this page's or a batch is under way. */
export function useJobRunning(): boolean {
  return useIsMutating({ mutationKey: jobKey }) > 0;
}

/**
 * Runs a mod as a request asks, and keeps its answer as the page's last run. A run the store
 * kept asks the store's reads again.
 */
export function useRunMod() {
  const setLast = useRepairStore((state) => state.setLast);
  const setLive = useRepairStore((state) => state.setLive);
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: runKey,
    mutationFn: async (request: RunRequest) => {
      setLive(() => startedRun());
      return unwrap(await commands.runMod(request));
    },
    onSuccess: (view) => {
      setLast(view);
      if (view.saved != null) {
        void queryClient.invalidateQueries({ queryKey: runsKey });
      }
    },
    onSettled: () => setLive(() => null),
  });
}

/**
 * Stops the run under way: the stage under way ends after its step (#ADR-0097), and the run
 * answers with its report, nothing written.
 */
export function useStopRun() {
  const setLive = useRepairStore((state) => state.setLive);
  return useMutation({
    mutationFn: async () => {
      setLive((live) => live && { ...live, stopping: true });
      unwrap(await commands.cancelRun());
    },
  });
}

/** A stage of the run under way, as an event of it tells. */
export function liveStage(event: StageEvent): LiveStage {
  return match(event)
    .with({ event: "started" }, () => ({ state: "running" as const }))
    .with({ event: "ended" }, ({ millis }) => ({ state: "ended" as const, millis }))
    .with({ event: "resumed" }, () => ({ state: "resumed" as const }))
    .exhaustive();
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
          const stage = liveStage(payload.event);
          return { ...live, stages: { ...live.stages, [payload.stage]: stage } };
        }),
      ),
      events.runFinding.listen(({ payload }) =>
        setLive((live) => live && { ...live, findings: [...live.findings, payload.finding] }),
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
