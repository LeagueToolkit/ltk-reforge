import { useMutation } from "@tanstack/react-query";

import { commands, type RunRequest } from "@/lib/bindings.gen";
import { unwrap } from "@/utils";

import { useRepairStore } from "../state/repairStore";

/** Runs a repair as a request asks, and keeps its answer as the page's last run. */
export function useRunRepair() {
  const setLast = useRepairStore((state) => state.setLast);
  return useMutation({
    mutationFn: async (request: RunRequest) => unwrap(await commands.runRepair(request)),
    onSuccess: (view) => setLast(view),
  });
}

/** The folder a path is in: the folder a package is written in when none is chosen. */
export function folderOf(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut > 0 ? path.slice(0, cut) : path;
}
