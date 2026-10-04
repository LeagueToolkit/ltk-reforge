import { useMutation } from "@tanstack/react-query";
import { useEffect } from "react";

import { commands, events, type RunRequest } from "@/lib/bindings.gen";
import { unwrap } from "@/utils";

import { useRepairStore } from "../state/repairStore";

/** Runs a mod as a request asks, and keeps its answer as the page's last run. */
export function useRunMod() {
  const setLast = useRepairStore((state) => state.setLast);
  const setLive = useRepairStore((state) => state.setLive);
  return useMutation({
    mutationFn: async (request: RunRequest) => {
      setLive(() => ({ started: Date.now(), stages: {}, findings: [], log: [] }));
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
