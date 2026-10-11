import { queryOptions, useQuery } from "@tanstack/react-query";

import { commands, type Settings } from "@/lib/bindings.gen";
import { useSettings } from "@/modules/settings";
import { queryFn } from "@/utils";

/** The input settings, in the order a query key holds them. */
function inputKey(settings: Settings) {
  return [
    settings["inputs.game"] ?? null,
    settings["inputs.hashes"] ?? null,
    settings["inputs.schema"] ?? null,
    settings["inputs.history"] ?? null,
  ] as const;
}

/**
 * The inputs the settings choose, opened. The key holds the input settings, so a changed one
 * opens them again. The backend keeps them open, so a later read with the same key is cheap.
 */
export const inputsQuery = (settings: Settings) =>
  queryOptions({
    queryKey: ["inputs", ...inputKey(settings)] as const,
    queryFn: queryFn(commands.openInputs),
    staleTime: Infinity,
    retry: false,
  });

/** The inputs the settings choose, opened, once the settings are read. */
export function useInputs() {
  const settings = useSettings();
  return useQuery({
    ...inputsQuery(settings.data ?? {}),
    enabled: settings.isSuccess,
  });
}
