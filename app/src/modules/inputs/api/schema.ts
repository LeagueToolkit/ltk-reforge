import {
  queryOptions,
  useIsMutating,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";

import { commands } from "@/lib/bindings.gen";
import { useSettings } from "@/modules/settings";
import { queryFn, unwrap } from "@/utils";

import { inputsQuery } from "./inputs";

/** The start of the key of every read of how the shared schema database stands. */
const schemaCheckKey = ["schema", "check"] as const;

/**
 * How the schema database the League Toolkit programs share stands against the one the LTK Meta
 * Wiki publishes. The key holds the user's own file, so the answer for the shared one is not
 * read while an own file is set.
 */
export const schemaCheckQuery = (own: string | null) =>
  queryOptions({
    queryKey: [...schemaCheckKey, own] as const,
    queryFn: queryFn(commands.schemaCheck),
    // The wiki publishes a database a few times a week.
    staleTime: 1000 * 60 * 30,
    retry: false,
  });

/** How the shared schema database stands, asked only while the runs read it. */
export function useSchemaCheck() {
  const settings = useSettings();
  const own = settings.data?.["inputs.schema"] || null;
  return useQuery({
    ...schemaCheckQuery(own),
    enabled: settings.isSuccess && own == null,
  });
}

/** The download of the schema database, a mutation under this key: a page left and come back to still sees it. */
const schemaUpdateKey = ["schema", "update"] as const;

/** Whether the schema database is downloading. */
export function useSchemaUpdating(): boolean {
  return useIsMutating({ mutationKey: schemaUpdateKey }) > 0;
}

/** Downloads the published schema database; the inputs open again to read it. */
export function useUpdateSchema() {
  const queryClient = useQueryClient();
  const settings = useSettings();
  return useMutation({
    mutationKey: schemaUpdateKey,
    mutationFn: async () => unwrap(await commands.updateSchema()),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: schemaCheckKey });
      void queryClient.invalidateQueries({ queryKey: inputsQuery(settings.data ?? {}).queryKey });
    },
  });
}
