import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { commands, events, type InstallProgress } from "@/lib/bindings.gen";
import { settingsKeys } from "@/modules/settings";
import { unwrap } from "@/utils";

/** The newest release of the census history that holds an index, read from GitHub. */
export const indexReleaseQuery = () =>
  queryOptions({
    queryKey: ["index", "release"] as const,
    queryFn: async () => unwrap(await commands.indexRelease()),
    // GitHub answers sixty unauthenticated reads an hour.
    staleTime: 1000 * 60 * 30,
    retry: false,
  });

/** The newest release of the census history that holds an index. */
export function useIndexRelease() {
  return useQuery(indexReleaseQuery());
}

/** Downloads and installs the newest index; the settings come back pointing at it. */
export function useInstallIndex() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => unwrap(await commands.installIndex()),
    onSuccess: (settings) => queryClient.setQueryData(settingsKeys.current(), settings),
  });
}

/** Stops the download under way. */
export function cancelIndex() {
  void commands.cancelIndex();
}

/** How far the download under way has got, from the backend's events. */
export function useIndexProgress(): InstallProgress | null {
  const [progress, setProgress] = useState<InstallProgress | null>(null);
  useEffect(() => {
    const unlisten = events.installProgress.listen((event) => setProgress(event.payload));
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);
  return progress;
}
