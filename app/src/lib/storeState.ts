import { queryOptions, useQuery } from "@tanstack/react-query";

import { queryFn } from "@/utils";

import { commands } from "./bindings.gen";
import { runsKey } from "./query";

/**
 * Whether the store of runs is open, and what a reset loses when another build made it. The
 * run page and the runs page both read it, and neither module imports the other, so it lives
 * here; under `runsKey`, a reset and a run kept ask it again.
 */
export function useStoreState() {
  return useQuery(
    queryOptions({
      queryKey: [...runsKey, "state"] as const,
      queryFn: queryFn(commands.storeState),
    }),
  );
}
