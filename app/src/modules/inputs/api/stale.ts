import { useIsFetching, useQueryClient } from "@tanstack/react-query";

import { useHashesCheck } from "./hashes";
import { useIndexBehind } from "./index";
import { useSchemaCheck } from "./schema";

/** The start of the key of every read that asks a server how an input stands. */
const checkKeys = [
  ["hashes", "check"],
  ["schema", "check"],
  ["index", "behind"],
  ["index", "release"],
] as const;

/** Asks every server again how the inputs stand, and says whether an answer is awaited. */
export function useCheckInputs() {
  const queryClient = useQueryClient();
  const checking =
    useIsFetching({
      predicate: (query) =>
        checkKeys.some(([area, what]) => query.queryKey[0] === area && query.queryKey[1] === what),
    }) > 0;
  const check = () => {
    for (const queryKey of checkKeys) {
      void queryClient.invalidateQueries({ queryKey });
    }
  };
  return { check, checking };
}

/** What a download would bring up to date, input by input. */
export interface StaleInputs {
  /** The hash tables the cache lacks or holds older than the newest release. */
  hashes: string[];
  /** The shared schema database, when it is absent or another than the one published. */
  schema: { held: boolean } | null;
  /** Whether the census history holds builds after the index. */
  history: boolean;
}

/**
 * The inputs a download would bring up to date (#ADR-0059): the hash tables and the schema
 * database, absent or behind what is published, and an index behind the census history. The
 * app downloads none of them unasked, and marks them instead.
 */
export function useStaleInputs(): StaleInputs {
  const hashes = useHashesCheck();
  const schema = useSchemaCheck();
  const index = useIndexBehind();
  return {
    hashes: hashes.data?.behind ?? [],
    schema: schema.data?.behind ? { held: schema.data.held } : null,
    history: index.data === true,
  };
}
