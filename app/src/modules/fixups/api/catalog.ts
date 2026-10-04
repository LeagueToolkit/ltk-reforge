import { queryOptions, useQuery } from "@tanstack/react-query";

import { commands } from "@/lib/bindings.gen";
import { queryFn } from "@/utils";

/** Every fixup of the library. The list is built into the app, so it never goes stale. */
export const catalogQuery = () =>
  queryOptions({
    queryKey: ["fixups", "catalog"] as const,
    queryFn: queryFn(commands.fixupCatalog),
    staleTime: Infinity,
  });

/** Every fixup of the library, in the order they run. */
export function useFixupCatalog() {
  return useQuery(catalogQuery());
}

/** Every check of the library. The list is built into the app, so it never goes stale. */
export const checksQuery = () =>
  queryOptions({
    queryKey: ["checks", "catalog"] as const,
    queryFn: queryFn(commands.checkCatalog),
    staleTime: Infinity,
  });

/** Every check of the library, in the order they run. */
export function useCheckCatalog() {
  return useQuery(checksQuery());
}
