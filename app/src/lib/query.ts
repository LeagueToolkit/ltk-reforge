import { QueryClient } from "@tanstack/react-query";

import type { RunRef } from "./bindings.gen";

/**
 * Every read of the store of runs. A run that is kept, a keep and a removal ask them all again:
 * the run page writes runs and the runs page reads them, and neither module imports the other.
 */
export const runsKey = ["runs"] as const;

/**
 * Every read a finding's detail asks of a kept run. A kept run never changes, so a change of the
 * store leaves them; a reset of the store, which numbers runs from 1 again, drops them.
 */
export const keptRunKey = ["kept-run"] as const;

/**
 * Every read of the run page's last run, which the next run replaces: the run page resets them
 * when a run ends, and a finding's detail asks under them.
 */
export const lastKey = ["last"] as const;

/**
 * Every read of the mod explored, which the next explore replaces: the Mod page resets them when
 * an explore ends, and a finding's detail asks under them.
 */
export const exploreKey = ["explore"] as const;

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

/**
 * The mutations of the backend's run job, which runs one at a time: the run page's run and a
 * batch. Each page asks whether either is under way by this prefix, and neither module imports
 * the other's key.
 */
export const jobKey = ["job"] as const;

/** The client every page shares. */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60,
      retry: 1,
    },
  },
});
