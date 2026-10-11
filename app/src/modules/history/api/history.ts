import { keepPreviousData, queryOptions, useQuery } from "@tanstack/react-query";

import { commands } from "@/lib/bindings.gen";
import { useSettings } from "@/modules/settings";
import { queryFn } from "@/utils";

/**
 * The query keys of the history's answers. Each holds the index the settings choose, so an
 * index installed or updated anew is asked again; an index never changes under one folder.
 */
const historyKeys = {
  all: (index: string | null) => ["history", index] as const,
  builds: (index: string | null) => [...historyKeys.all(index), "builds"] as const,
  lookup: (index: string | null, query: string, at: number | null) =>
    [...historyKeys.all(index), "lookup", query, at] as const,
  suggest: (index: string | null, prefix: string) =>
    [...historyKeys.all(index), "suggest", prefix] as const,
  object: (index: string | null, bin: string, entry: string, at: number) =>
    [...historyKeys.all(index), "object", bin, entry, at] as const,
};

/** The folder of the index the settings choose, null for none. */
function useIndex(): string | null {
  return useSettings().data?.["inputs.history"] ?? null;
}

/** The builds of the history, in position order, and its head. */
export function useHistoryBuilds(enabled: boolean) {
  const index = useIndex();
  return useQuery(
    queryOptions({
      queryKey: historyKeys.builds(index),
      queryFn: queryFn(commands.historyBuilds),
      staleTime: Infinity,
      retry: false,
      enabled,
    }),
  );
}

/** A path, or its hash, looked up at the build at `at`. Off while no query is asked. */
export function useHistoryLookup(query: string | null, at: number | null) {
  const index = useIndex();
  return useQuery(
    queryOptions({
      queryKey: historyKeys.lookup(index, query ?? "", at),
      queryFn: queryFn(() => commands.historyLookup(query ?? "", at)),
      staleTime: Infinity,
      retry: false,
      enabled: query != null,
    }),
  );
}

/** The paths the hash tables spell that start with a prefix; the last answer stays while the next is asked. */
export function useHistorySuggest(prefix: string) {
  const index = useIndex();
  return useQuery(
    queryOptions({
      queryKey: historyKeys.suggest(index, prefix),
      queryFn: queryFn(() => commands.historySuggest(prefix)),
      staleTime: Infinity,
      retry: false,
      enabled: prefix.trim().length >= 3,
      placeholderData: keepPreviousData,
    }),
  );
}

/** An object of a bin at the build at `at`, as text; null when that build holds none. */
export function useHistoryObject(bin: string, entry: string, at: number | null) {
  const index = useIndex();
  return useQuery(
    queryOptions({
      queryKey: historyKeys.object(index, bin, entry, at ?? -1),
      queryFn: queryFn(() => commands.historyObject(bin, entry, at ?? 0)),
      staleTime: Infinity,
      retry: false,
      enabled: bin !== "" && entry !== "" && at != null,
    }),
  );
}
