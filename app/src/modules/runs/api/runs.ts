import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { commands, type StageName } from "@/lib/bindings.gen";
import { runsKey } from "@/lib/query";
import { mutationFn, queryFn } from "@/utils";

/** The query keys of the store of runs, all under `runsKey`. */
export const runsKeys = {
  list: () => [...runsKey, "list"] as const,
  view: (id: number) => [...runsKey, "view", id] as const,
  files: (id: number, stage: StageName) => [...runsKey, "files", id, stage] as const,
  file: (id: number, content: string) => [...runsKey, "file", id, content] as const,
  report: (id: number) => [...runsKey, "report", id] as const,
  store: () => [...runsKey, "store"] as const,
};

/** The runs the store keeps, newest first. */
export function useRuns() {
  return useQuery(queryOptions({ queryKey: runsKeys.list(), queryFn: queryFn(commands.listRuns) }));
}

/** What the views drew of a kept run as it ended. A kept run never changes. */
export function useRunView(id: number) {
  return useQuery(
    queryOptions({
      queryKey: runsKeys.view(id),
      queryFn: queryFn(() => commands.runView(id)),
      staleTime: Infinity,
    }),
  );
}

/** The report a kept run's package embeds, as JSON. */
export function useRunReport(id: number) {
  return useQuery(
    queryOptions({
      queryKey: runsKeys.report(id),
      queryFn: queryFn(() => commands.runReport(id)),
      staleTime: Infinity,
    }),
  );
}

/** What the store holds and its bytes on disk. */
export function useStoreView() {
  return useQuery(
    queryOptions({ queryKey: runsKeys.store(), queryFn: queryFn(commands.storeView) }),
  );
}

/** A change of the store, which asks every read of it again. */
function useStoreChange<V>(change: (variables: V) => ReturnType<typeof commands.keepRun>) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: mutationFn(change),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: runsKey }),
  });
}

/** Marks a kept run so that the store's bound does not remove it, or unmarks it. */
export function useKeepRun() {
  return useStoreChange(({ id, kept }: { id: number; kept: boolean }) =>
    commands.keepRun(id, kept),
  );
}

/** Removes a kept run. */
export function useRemoveRun() {
  return useStoreChange((id: number) => commands.removeRun(id));
}

/** Removes every kept run. */
export function useRemoveAllRuns() {
  return useStoreChange<void>(() => commands.removeAllRuns());
}

/** Shows the package a kept run wrote in its folder. */
export function useRevealRun() {
  return useMutation({
    mutationFn: mutationFn((id: number) => commands.revealPlace({ run: { id } })),
  });
}
