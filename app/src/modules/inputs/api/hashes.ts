import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { commands, events, type HashesProgress } from "@/lib/bindings.gen";
import { useSettings } from "@/modules/settings";
import { queryFn, unwrap } from "@/utils";

import { inputsQuery } from "./inputs";

/**
 * What an update of the hash cache in `dir` would download, against the newest release of the
 * tables. The key holds the folder, so a folder chosen anew is asked about again.
 */
export const hashesCheckQuery = (dir: string | null) =>
  queryOptions({
    queryKey: ["hashes", "check", dir] as const,
    queryFn: queryFn(commands.hashesCheck),
    // GitHub answers sixty unauthenticated reads an hour, and the tables change weekly.
    staleTime: 1000 * 60 * 30,
    retry: false,
  });

/** What an update of the hash tables the settings choose would download. */
export function useHashesCheck() {
  const settings = useSettings();
  return useQuery({
    ...hashesCheckQuery(settings.data?.["inputs.hashes"] ?? null),
    enabled: settings.isSuccess,
  });
}

/** Downloads the tables the cache lacks or holds older; the inputs open again to read them. */
export function useUpdateHashes() {
  const queryClient = useQueryClient();
  const settings = useSettings();
  return useMutation({
    mutationFn: async () => unwrap(await commands.updateHashes()),
    onSuccess: () => {
      const chosen = settings.data ?? {};
      const check = hashesCheckQuery(chosen["inputs.hashes"] ?? null);
      void queryClient.invalidateQueries({ queryKey: check.queryKey });
      void queryClient.invalidateQueries({ queryKey: inputsQuery(chosen).queryKey });
    },
  });
}

/** Stops the download under way. */
export function cancelHashes() {
  void commands.cancelHashes();
}

/** How far the download under way has got, from the backend's events. */
export function useHashesProgress(): HashesProgress | null {
  const [progress, setProgress] = useState<HashesProgress | null>(null);
  useEffect(() => {
    const unlisten = events.hashesProgress.listen((event) => setProgress(event.payload));
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);
  return progress;
}
