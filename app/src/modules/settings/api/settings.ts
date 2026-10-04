import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { commands, type Settings } from "@/lib/bindings.gen";
import { mutationFn, queryFn } from "@/utils";

/** The query keys of the settings. */
export const settingsKeys = {
  all: ["settings"] as const,
  current: () => [...settingsKeys.all, "current"] as const,
};

/** The settings as they stand. The app alone changes them, so they never go stale. */
export const settingsQuery = () =>
  queryOptions({
    queryKey: settingsKeys.current(),
    queryFn: queryFn(commands.getSettings),
    staleTime: Infinity,
  });

/** The settings as they stand. */
export function useSettings() {
  return useQuery(settingsQuery());
}

/** Saves the settings; the query holds them as saved. */
export function useSaveSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: mutationFn(commands.saveSettings),
    onSuccess: (saved: Settings) => queryClient.setQueryData(settingsKeys.current(), saved),
  });
}
