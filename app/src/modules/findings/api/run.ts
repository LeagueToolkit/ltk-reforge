import { queryOptions, useQuery } from "@tanstack/react-query";
import { createContext, useContext, useMemo } from "react";

import { commands, type RunRef } from "@/lib/bindings.gen";
import { exploreKey, keptRunKey, lastKey } from "@/lib/query";
import { queryFn } from "@/utils";

/**
 * The run whose findings a page shows, which a finding's detail asks about: its names spell the
 * hashes, and its package gives the type check's places (#ADR-0103). None while a run is under
 * way, whose findings are not yet any run's the backend holds.
 */
export const RunRefContext = createContext<RunRef | null>(null);

/** The run whose findings are shown, when the page names one. */
export function useRunRef(): RunRef | null {
  return useContext(RunRefContext);
}

/** The run page's last run. */
export const LAST_RUN: RunRef = { of: "last" };

/** The mod explored. */
export const EXPLORED: RunRef = { of: "explored" };

/** A kept run, by its number. */
export function keptRun(id: number): RunRef {
  return { of: "kept", id };
}

/**
 * The key a run's reads are kept under: the next run or explore resets the last run's and the
 * explored mod's, and a kept run's stay until the store is reset.
 */
export function runRefKey(run: RunRef): readonly unknown[] {
  switch (run.of) {
    case "kept":
      return [...keptRunKey, run.id];
    case "last":
      return lastKey;
    case "explored":
      return exploreKey;
  }
}

/** The most hashes one ask spells, as the backend's `places::MAX_SPELLED` bounds it. */
export const MAX_SPELLED = 4096;

/** The hashes to ask for: each once, in order, up to the bound. */
export function askedHashes(hashes: readonly string[]): string[] {
  return [...new Set(hashes)].sort().slice(0, MAX_SPELLED);
}

/**
 * A name for each hash the run's names or the inputs' tables spell. Asked once per run and set
 * of hashes; a hash no name spells is absent.
 */
export function useSpelled(run: RunRef | null, hashes: readonly string[]): Record<string, string> {
  const asked = useMemo(() => askedHashes(hashes), [hashes]);
  const spelled = useQuery(
    queryOptions({
      queryKey: [...(run ? runRefKey(run) : ["none"]), "spelled", ...asked],
      queryFn: queryFn(() => commands.spellHashes(run ?? LAST_RUN, asked)),
      enabled: run != null && asked.length > 0,
      staleTime: Infinity,
      retry: false,
    }),
  );
  return spelled.data ?? {};
}

/**
 * Every value the type check counts in the run's package, with its place (#ADR-0103). The
 * backend reads the package again, which takes seconds on a large port, so it is asked only
 * once a row's places are opened, and once per run.
 */
export function useTypePlaces(run: RunRef) {
  return useQuery(
    queryOptions({
      queryKey: [...runRefKey(run), "type-places"],
      queryFn: queryFn(() => commands.typePlaces(run)),
      staleTime: Infinity,
      retry: false,
    }),
  );
}
