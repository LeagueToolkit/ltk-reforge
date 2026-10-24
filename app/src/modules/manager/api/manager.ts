import { queryOptions, useQuery } from "@tanstack/react-query";

import { commands } from "@/lib/bindings.gen";
import { queryFn } from "@/utils";

/** LTK Manager's library, read from its data folder (#ADR-0106). */
export const managerLibraryQuery = queryOptions({
  queryKey: ["manager", "library"] as const,
  queryFn: queryFn(commands.managerLibrary),
});

/** The manager's library as it stands. */
export function useManagerLibrary() {
  return useQuery(managerLibraryQuery);
}
