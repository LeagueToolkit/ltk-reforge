import {
  queryOptions,
  useIsMutating,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useEffect, useState } from "react";

import {
  type ArchiveProgress,
  commands,
  events,
  type InstallProgress,
  type UpdateStep,
} from "@/lib/bindings.gen";
import { settingsKeys, useSettings } from "@/modules/settings";
import { queryFn, unwrap } from "@/utils";

import { storageKey } from "./storage";

/**
 * The work that writes the indexes, the clones or the downloads. The backend runs one at a
 * time, so each is a mutation under this key and a button of one is off while another runs.
 */
const indexJobKey = ["index-job"] as const;

/** A kind of work on the history. */
export type IndexJobKind =
  | "install"
  | "update"
  | "bootstrap"
  | "remove-index"
  | "remove-census"
  | "remove-downloads";

/** Whether work on the history is under way: any, or of one kind. A page left and come back to still answers. */
export function useIndexJobRunning(kind?: IndexJobKind): boolean {
  const mutationKey = kind ? [...indexJobKey, kind] : indexJobKey;
  return useIsMutating({ mutationKey }) > 0;
}

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
    mutationKey: [...indexJobKey, "install"],
    mutationFn: async () => unwrap(await commands.installIndex()),
    onSuccess: (settings) => queryClient.setQueryData(settingsKeys.current(), settings),
    // A download that stopped is kept on disk.
    onSettled: () => queryClient.invalidateQueries({ queryKey: storageKey }),
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

/**
 * Whether the census history holds builds after the head of the index in `index`. The key holds
 * the folder, so an index updated or chosen anew is asked about again.
 */
export const indexBehindQuery = (index: string | null) =>
  queryOptions({
    queryKey: ["index", "behind", index] as const,
    queryFn: queryFn(commands.indexBehind),
    // The census history takes a build or two a week.
    staleTime: 1000 * 60 * 30,
    retry: false,
    enabled: index != null,
  });

/** Whether the census history holds builds after the head of the index the settings choose. */
export function useIndexBehind() {
  const settings = useSettings();
  return useQuery(indexBehindQuery(settings.data?.["inputs.history"] ?? null));
}

/** The app's clone of the census history: none, fetched as far as updates read it, or whole. */
export const censusCloneQuery = () =>
  queryOptions({
    queryKey: ["census", "clone"] as const,
    queryFn: queryFn(commands.censusClone),
    // Only the app changes it, and each change answers it anew.
    staleTime: Infinity,
  });

/** Fetches the builds after the index's head and appends them; the settings come back pointing at the new index. */
export function useUpdateIndex() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: [...indexJobKey, "update"],
    mutationFn: async () => unwrap(await commands.updateIndex()),
    onSuccess: (settings) => queryClient.setQueryData(settingsKeys.current(), settings),
    onSettled: () => {
      // A first update makes the app's clone, and a stopped one keeps what it fetched.
      void queryClient.invalidateQueries({ queryKey: censusCloneQuery().queryKey });
      void queryClient.invalidateQueries({ queryKey: storageKey });
    },
  });
}

/** The app's clone of the census history. */
export function useCensusClone() {
  return useQuery(censusCloneQuery());
}

/** Downloads the newest release's archive of the census history and makes the clone of it. */
export function useBootstrapCensus() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: [...indexJobKey, "bootstrap"],
    mutationFn: async () => unwrap(await commands.bootstrapCensus()),
    onSuccess: (clone) => queryClient.setQueryData(censusCloneQuery().queryKey, clone),
    onSettled: () => queryClient.invalidateQueries({ queryKey: storageKey }),
  });
}

/** Removes every index the app installed; the settings come back choosing none of them. */
export function useRemoveIndex() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: [...indexJobKey, "remove-index"],
    mutationFn: async () => unwrap(await commands.removeIndex()),
    onSuccess: (settings) => queryClient.setQueryData(settingsKeys.current(), settings),
    // A folder that did not go is still on disk.
    onSettled: () => queryClient.invalidateQueries({ queryKey: storageKey }),
  });
}

/** Removes the app's clones of the census history. */
export function useRemoveCensus() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: [...indexJobKey, "remove-census"],
    mutationFn: async () => unwrap(await commands.removeCensus()),
    onSuccess: (clone) => queryClient.setQueryData(censusCloneQuery().queryKey, clone),
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: censusCloneQuery().queryKey });
      void queryClient.invalidateQueries({ queryKey: storageKey });
    },
  });
}

/** Removes the downloads kept to resume. */
export function useRemoveDownloads() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: [...indexJobKey, "remove-downloads"],
    mutationFn: async () => unwrap(await commands.removeDownloads()),
    onSettled: () => queryClient.invalidateQueries({ queryKey: storageKey }),
  });
}

/** How far the bootstrap under way has got, from the backend's events. */
export function useArchiveProgress(): ArchiveProgress | null {
  const [progress, setProgress] = useState<ArchiveProgress | null>(null);
  useEffect(() => {
    const unlisten = events.archiveProgress.listen((event) => setProgress(event.payload));
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);
  return progress;
}

/** The step of the update under way, from the backend's events. */
export function useIndexUpdateStep(): UpdateStep | null {
  const [step, setStep] = useState<UpdateStep | null>(null);
  useEffect(() => {
    const unlisten = events.indexUpdateProgress.listen((event) => setStep(event.payload.step));
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);
  return step;
}
