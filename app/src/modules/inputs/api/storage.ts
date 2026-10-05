import { queryOptions, useMutation, useQuery } from "@tanstack/react-query";

import { commands, type Place } from "@/lib/bindings.gen";
import { useSettings } from "@/modules/settings";
import { queryFn, unwrap } from "@/utils";

/** Every read of what the app keeps on disk, to ask them all again after a download or a removal. */
export const storageKey = ["storage"] as const;

/**
 * What the app keeps on disk for the history. The key holds the index the settings choose, so an
 * index chosen anew is measured again.
 */
export const storageQuery = (index: string | null) =>
  queryOptions({
    queryKey: [...storageKey, index] as const,
    queryFn: queryFn(commands.storage),
    // Only the app changes it, and each change asks anew.
    staleTime: Infinity,
  });

/** What the app keeps on disk for the history, once the settings are read. */
export function useStorage() {
  const settings = useSettings();
  return useQuery({
    ...storageQuery(settings.data?.["inputs.history"] ?? null),
    enabled: settings.isSuccess,
  });
}

/** Shows a folder of the inputs in the system's file manager. */
export function useReveal() {
  return useMutation({
    mutationFn: async (place: Place) => unwrap(await commands.revealPlace(place)),
  });
}
