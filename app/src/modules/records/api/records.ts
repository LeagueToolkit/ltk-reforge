import { queryOptions, useQuery } from "@tanstack/react-query";

import { commands } from "@/lib/bindings.gen";
import { queryFn } from "@/utils";

/** The query keys of the records. The records are built into the app, so none goes stale. */
const recordsKeys = {
  list: () => ["records", "list"] as const,
  record: (code: string) => ["records", "record", code] as const,
};

/** Every record bundled into the app, in series order then number order. */
export function useRecords() {
  return useQuery(
    queryOptions({
      queryKey: recordsKeys.list(),
      queryFn: queryFn(commands.records),
      staleTime: Infinity,
    }),
  );
}

/** A bundled record by its code, with or without its `#`; null for a code the bundle lacks. */
export function useRecord(code: string) {
  return useQuery(
    queryOptions({
      queryKey: recordsKeys.record(code),
      queryFn: queryFn(() => commands.record(code)),
      staleTime: Infinity,
    }),
  );
}
