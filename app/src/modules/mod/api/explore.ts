import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { commands } from "@/lib/bindings.gen";
import { exploreKey } from "@/lib/query";
import type { FileQuery } from "@/modules/files";
import { unwrap } from "@/utils";

import { useModStore } from "../state/modStore";

/**
 * Explores the archive picked through `load`, and keeps the view as the page's. The reads of
 * the mod explored before it are asked again: the backend holds this one in its place.
 */
export function useExploreMod() {
  const setExplored = useModStore((state) => state.setExplored);
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: [...exploreKey, "mod"],
    mutationFn: async (archive: string) => {
      const view = unwrap(await commands.exploreMod(archive));
      return { archive, view };
    },
    onSuccess: async (explored) => {
      setExplored(explored);
      await queryClient.resetQueries({ queryKey: exploreKey });
    },
  });
}

/** The files of the mod explored at `load`. Asked once per explore, while `enabled`. */
export function useExploreFiles(enabled: boolean) {
  return useQuery(
    queryOptions({
      queryKey: [...exploreKey, "files"],
      queryFn: async () => unwrap(await commands.exploreFiles()),
      enabled,
      staleTime: Infinity,
    }),
  );
}

/** One file of the mod explored, by the id of its bytes, shown by its kind. */
export const exploreFileQuery = (content: string): FileQuery => ({
  queryKey: [...exploreKey, "file", content],
  queryFn: async () => unwrap(await commands.exploreFile(content)),
  run: { of: "explored" },
});

/** The mod explored's archive and the standard settings, for the run page. */
export function useExploreRun() {
  return useMutation({
    mutationFn: async () => unwrap(await commands.exploreRun()),
  });
}
