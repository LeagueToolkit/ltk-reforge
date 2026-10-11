import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { commands, type StageName, type StoreState } from "@/lib/bindings.gen";
import { keptRunKey, runsKey } from "@/lib/query";
import type { FileQuery, FilesQuery } from "@/modules/files";
import { mutationFn, queryFn, type Result } from "@/utils";

import type { Opened } from "./drop";

/** The query keys of the store of runs, all under `runsKey`. */
export const runsKeys = {
  list: () => [...runsKey, "list"] as const,
  view: (id: number) => [...runsKey, "view", id] as const,
  files: (id: number, stage: StageName) => [...runsKey, "files", id, stage] as const,
  file: (id: number, content: string) => [...runsKey, "file", id, content] as const,
  report: (id: number) => [...runsKey, "report", id] as const,
  again: (id: number) => [...runsKey, "again", id] as const,
  compare: (a: number, b: number) => [...runsKey, "compare", a, b] as const,
  compareFiles: (a: number, b: number, stage: StageName) =>
    [...runsKey, "compare-files", a, b, stage] as const,
  ingest: (sha256: string) => [...runsKey, "ingest", sha256] as const,
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

/**
 * What a kept run was run on and with, for the run page to run it again. Asked again each time
 * it is shown: the archive may have left its path since.
 */
export function useRunAgain(id: number) {
  return useQuery(
    queryOptions({
      queryKey: runsKeys.again(id),
      queryFn: queryFn(() => commands.runSettings(id)),
    }),
  );
}

/** The read of a kept run's files at a stage. */
export const runFilesQuery = (id: number, stage: StageName): FilesQuery => ({
  queryKey: runsKeys.files(id, stage),
  queryFn: queryFn(() => commands.runFiles(id, stage)),
});

/** The files of a kept run at a stage, each marked against the stage before. */
export function useRunFiles(id: number, stage: StageName) {
  return useQuery({ ...runFilesQuery(id, stage), staleTime: Infinity });
}

/** Two kept runs compared, `a` against `b`. Kept runs never change. */
export function useCompare(a: number, b: number) {
  return useQuery(
    queryOptions({
      queryKey: runsKeys.compare(a, b),
      queryFn: queryFn(() => commands.compareRuns(a, b)),
      staleTime: Infinity,
    }),
  );
}

/** The files of kept run `b` at a stage, each marked against kept run `a`'s. */
export function useCompareFiles(a: number, b: number, stage: StageName) {
  return useQuery(
    queryOptions({
      queryKey: runsKeys.compareFiles(a, b, stage),
      queryFn: queryFn(() => commands.compareFiles(a, b, stage)),
      staleTime: Infinity,
    }),
  );
}

/** One file of a kept run, by the id of its bytes, shown by its kind. */
export const runFileQuery = (id: number, content: string): FileQuery => ({
  queryKey: runsKeys.file(id, content),
  queryFn: queryFn(() => commands.runFile(content)),
  run: { of: "kept", id },
});

/** What the store holds and its bytes on disk. Asked only while `enabled`: the store is open. */
export function useStoreView(enabled: boolean) {
  return useQuery(
    queryOptions({ queryKey: runsKeys.store(), queryFn: queryFn(commands.storeView), enabled }),
  );
}

/** A change of the store, which asks every read of it again. */
function useStoreChange<V, T = null>(change: (variables: V) => Promise<Result<T>>) {
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

/** Removes every kept run of the mod whose archive has this SHA-256. */
export function useRemoveModRuns() {
  return useStoreChange((sha256: string) => commands.removeModRuns(sha256));
}

/** Clears the archive data of the mod whose archive has this SHA-256 from the ingest tier. */
export function useClearModIngest() {
  return useStoreChange((sha256: string) => commands.clearModIngest(sha256));
}

/** Clears the archive data of every mod no kept run is of from the ingest tier. */
export function useClearUnusedIngest() {
  return useStoreChange<void>(() => commands.clearUnusedIngest());
}

/**
 * The bytes the ingest tier holds of a mod: what clearing its archive data frees at most.
 * Asked only while `enabled`, since it reads every entry of the tier.
 */
export function useModIngestBytes(sha256: string, enabled: boolean) {
  return useQuery(
    queryOptions({
      queryKey: runsKeys.ingest(sha256),
      queryFn: queryFn(() => commands.modIngestBytes(sha256)),
      enabled,
    }),
  );
}

/** Removes every kept run. */
export function useRemoveAllRuns() {
  return useStoreChange<void>(() => commands.removeAllRuns());
}

/** Removes the store's rows and opens it anew, and answers its state. */
export function useResetStore() {
  const queryClient = useQueryClient();
  return useStoreChange<void, StoreState>(async () => {
    const reset = await commands.resetStore();
    queryClient.removeQueries({ queryKey: keptRunKey });
    return reset;
  });
}

/** Opens a project file or a package's evidence as a kept run, and answers its number. */
export function useOpenRun() {
  return useStoreChange(({ kind, path }: Opened) =>
    kind === "project" ? commands.importProject(path) : commands.openPackage(path),
  );
}

/** Writes a kept run as a project file at a path, and answers the file's size. */
export function useExportProject() {
  return useMutation({
    mutationFn: mutationFn(({ id, path }: { id: number; path: string }) =>
      commands.exportProject(id, path),
    ),
  });
}

/** Shows the package a kept run wrote in its folder. */
export function useRevealRun() {
  return useMutation({
    mutationFn: mutationFn((id: number) => commands.revealPlace({ run: { id } })),
  });
}
