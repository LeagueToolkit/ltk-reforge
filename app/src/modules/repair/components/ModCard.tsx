import { queryOptions, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useCallback, useEffect } from "react";

import { Button, FailureNote, SectionCard } from "@/components";
import { m } from "@/i18n";
import { commands } from "@/lib/bindings.gen";
import { pickFile } from "@/modules/inputs";
import { useSettings } from "@/modules/settings";
import { queryFn, twMerge } from "@/utils";

import { ARCHIVE_EXTENSIONS, useArchiveDrop } from "../api/drop";
import { folderOf, useRunRunning } from "../api/run";
import { useRepairStore } from "../state/repairStore";

/** The folder a package is written in: the chosen one, else the mod's own. */
export function useOutputFolder(): string | null {
  const settings = useSettings();
  const archive = useRepairStore((state) => state.archive);
  const chosen = settings.data?.["output.folder"];
  if (chosen) {
    return chosen;
  }
  return archive ? folderOf(archive) : null;
}

const peekKey = ["mod", "peek"] as const;

/** The archive read alone. The file is read once per pick. */
const peekQuery = (archive: string | null) =>
  queryOptions({
    queryKey: [...peekKey, archive] as const,
    queryFn: queryFn(() => commands.peekMod(archive ?? "")),
    enabled: archive !== null,
    staleTime: Infinity,
    retry: false,
  });

/** The archive read alone, kept in the page's state while it is the one picked. */
function usePeek(archive: string | null) {
  const setPeek = useRepairStore((state) => state.setPeek);
  const query = useQuery(peekQuery(archive));
  useEffect(() => {
    setPeek(query.data ?? null);
  }, [query.data, setPeek]);
  return query;
}

/**
 * The mod to repair, picked with a dialog or dropped on the window: as its archive names it
 * when it is picked, as the last run named it after, and where its package goes. The mod does
 * not change during a run, whose result would otherwise be shown as the new mod's.
 */
export function ModCard() {
  const archive = useRepairStore((state) => state.archive);
  const setArchive = useRepairStore((state) => state.setArchive);
  const running = useRunRunning();
  const ran = useRepairStore((state) => state.last?.meta);
  const built = useRepairStore(
    (state) => (state.live ? state.live.package : state.last?.package)?.thumbnail,
  );
  const settings = useSettings();
  const queryClient = useQueryClient();
  const peek = usePeek(archive);
  const meta = ran ?? peek.data?.meta;
  const thumbnail = built ?? peek.data?.thumbnail;
  const chosenOut = settings.data?.["output.folder"];

  const takeMod = useCallback(
    async (picked: string) => {
      // The file at the path may have changed since it was last read.
      await queryClient.invalidateQueries({ queryKey: peekKey });
      setArchive(picked);
    },
    [queryClient, setArchive],
  );
  const chooseMod = async () => {
    const picked = await pickFile([...ARCHIVE_EXTENSIONS], m.repair_archive_filter_label());
    if (picked) {
      await takeMod(picked);
    }
  };
  // The run page runs one mod: the first archive dropped.
  const onDrop = useCallback(
    ([first]: string[]) => {
      if (!running && first) {
        void takeMod(first);
      }
    },
    [running, takeMod],
  );
  const over = useArchiveDrop(onDrop);

  return (
    <SectionCard title={m.repair_mod_title()}>
      <div
        className={twMerge(
          "flex items-start justify-between gap-4 rounded-md",
          over && !running && "outline-2 outline-offset-8 outline-accent-500 outline-dashed",
        )}
        data-ui="ModCard:mod"
      >
        {thumbnail && (
          <img
            src={thumbnail}
            alt={m.repair_mod_thumbnail_alt()}
            className="h-16 w-16 shrink-0 rounded-md border border-surface-700 object-cover"
          />
        )}
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          {meta?.name && <p className="text-sm text-surface-50">{meta.name}</p>}
          {(meta?.author || meta?.version) && (
            <p className="text-xs text-surface-300">
              {[
                meta.author && m.repair_mod_by_label({ author: meta.author }),
                meta.version && m.repair_mod_version_label({ version: meta.version }),
              ]
                .filter(Boolean)
                .join(", ")}
            </p>
          )}
          {peek.data && peek.data.characters.length > 0 && (
            <p className="text-xs text-surface-300" data-ui="ModCard:characters">
              {peek.data.characters.map((character, index) => (
                <span key={character.name}>
                  {index > 0 && " · "}
                  {character.name}{" "}
                  <span className="text-surface-400">
                    {m.repair_mod_skins_label({ slots: character.slots.join(", ") })}
                  </span>
                </span>
              ))}
            </p>
          )}
          {archive && <p className="font-mono text-xs break-all text-surface-300">{archive}</p>}
          {peek.isError && <FailureNote failure={peek.error} />}
          {!archive && (
            <p className="text-sm text-surface-400 select-none">
              {over && !running ? m.repair_mod_drop_label() : m.repair_mod_none()}
            </p>
          )}
        </div>
        <Button size="sm" variant="outline" disabled={running} onClick={() => void chooseMod()}>
          {m.repair_mod_choose_action()}
        </Button>
      </div>
      <Link
        to="/settings"
        className="text-xs break-all text-surface-400 underline-offset-2 hover:text-surface-200 hover:underline"
        data-ui="ModCard:out"
      >
        {chosenOut ? m.repair_out_folder_label({ folder: chosenOut }) : m.repair_out_beside_label()}
      </Link>
    </SectionCard>
  );
}
