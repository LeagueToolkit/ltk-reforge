import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { commands, type InstallView, type Settings } from "@/lib/bindings.gen";
import { settingsKeys } from "@/modules/settings";
import { mutationFn, queryFn, type Result } from "@/utils";

/** The key of the list of installs. */
const installsKey = ["installs"] as const;

/**
 * The game installs on this machine and the ones added, the chosen one first (#ADR-0101). The
 * backend sorts it. Nothing but the app changes the added ones, so it is asked for again only
 * on request.
 */
export const installsQuery = () =>
  queryOptions({
    queryKey: installsKey,
    queryFn: queryFn(commands.installs),
    staleTime: Infinity,
  });

/** The game installs on this machine and the ones added. */
export function useInstalls() {
  return useQuery(installsQuery());
}

/**
 * A change to the list that answers the list. The settings it saved are read again, so a later
 * save of another setting does not write the older list back.
 */
function useListChange(command: (path: string) => Promise<Result<InstallView[]>>) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: mutationFn(command),
    onSuccess: (list: InstallView[]) => {
      queryClient.setQueryData(installsKey, list);
      void queryClient.invalidateQueries({ queryKey: settingsKeys.current() });
    },
  });
}

/** Adds a folder to the installs; refused when it is no install. */
export function useAddInstall() {
  return useListChange(commands.addInstall);
}

/** Takes an added install off the list. */
export function useRemoveInstall() {
  return useListChange(commands.removeInstall);
}

/** Chooses a listed install as the one a run reads. */
export function useChooseInstall() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: mutationFn(commands.chooseInstall),
    onSuccess: (saved: Settings) => {
      queryClient.setQueryData(settingsKeys.current(), saved);
      void queryClient.invalidateQueries({ queryKey: installsKey });
    },
  });
}
