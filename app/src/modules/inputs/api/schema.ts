import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { commands } from "@/lib/bindings.gen";
import { useSettings } from "@/modules/settings";
import { queryFn, unwrap } from "@/utils";

import { inputsQuery } from "./inputs";

/**
 * How the schema database the League Toolkit programs share stands against the one the LTK Meta
 * Wiki publishes.
 */
export const schemaCheckQuery = () =>
  queryOptions({
    queryKey: ["schema", "check"] as const,
    queryFn: queryFn(commands.schemaCheck),
    // The wiki publishes a database a few times a week.
    staleTime: 1000 * 60 * 30,
    retry: false,
  });

/** How the shared schema database stands, asked only while the runs read it. */
export function useSchemaCheck() {
  const settings = useSettings();
  return useQuery({
    ...schemaCheckQuery(),
    enabled: settings.isSuccess && !settings.data["inputs.schema"],
  });
}

/** Downloads the published schema database; the inputs open again to read it. */
export function useUpdateSchema() {
  const queryClient = useQueryClient();
  const settings = useSettings();
  return useMutation({
    mutationFn: async () => unwrap(await commands.updateSchema()),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: schemaCheckQuery().queryKey });
      void queryClient.invalidateQueries({ queryKey: inputsQuery(settings.data ?? {}).queryKey });
    },
  });
}
