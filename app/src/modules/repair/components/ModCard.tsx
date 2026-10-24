import { queryOptions, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { type ReactNode, useCallback, useEffect } from "react";

import { Button, FailureNote, SectionCard } from "@/components";
import { m } from "@/i18n";
import { type CharacterPeek, commands, type ModView, type PackageFormat } from "@/lib/bindings.gen";
import { pickFile } from "@/modules/inputs";
import { DEFAULT_PACKAGE_FORMAT, useSettings } from "@/modules/settings";
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

/** The form a package is written in: the chosen one, else the default (#ADR-0105). */
export function usePackageFormat(): PackageFormat {
  const settings = useSettings();
  return settings.data?.["output.format"] ?? DEFAULT_PACKAGE_FORMAT;
}

/** The key of every read of an archive alone. */
export const peekKey = ["mod", "peek"] as const;

/** The archive read alone. The file is read once per pick. */
export const peekQuery = (archive: string | null) =>
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
 * Takes an archive picked or dropped as the one a page shows, reading the file at the path
 * again: it may have changed since it was last read.
 */
export function useTakeArchive(take: (archive: string) => void) {
  const queryClient = useQueryClient();
  return useCallback(
    async (picked: string) => {
      await queryClient.invalidateQueries({ queryKey: peekKey });
      take(picked);
    },
    [queryClient, take],
  );
}

/** Asks for a mod archive with the system's dialog. Null when the person cancels. */
export async function chooseArchive(): Promise<string | null> {
  return await pickFile([...ARCHIVE_EXTENSIONS], m.repair_archive_filter_label());
}

export interface ModFaceProps {
  /** The archive picked. */
  archive: string | null;
  /** The mod as its archive or a run names it. */
  meta: ModView | null | undefined;
  /** Its thumbnail as a `data:` URL. */
  thumbnail: string | null | undefined;
  /** The characters its paths name, with their slots. */
  characters: CharacterPeek[];
  /** Why the archive does not read alone, when it does not. */
  failure?: unknown;
  /** Whether a mod archive is held over the window and would be taken. */
  over: boolean;
  /** The actions beside the mod. */
  actions: ReactNode;
}

/**
 * A mod as it is picked: its thumbnail, name, author and version, the characters its paths
 * name with their slots, and the archive's path, outlined while an archive is held over the
 * window.
 */
export function ModFace({
  archive,
  meta,
  thumbnail,
  characters,
  failure,
  over,
  actions,
}: ModFaceProps) {
  return (
    <div
      className={twMerge(
        // The actions go under the mod when beside it they would squeeze its path.
        "flex flex-wrap items-start justify-between gap-4 rounded-md",
        over && "outline-2 outline-offset-8 outline-accent-500 outline-dashed",
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
      <div className="flex min-w-64 flex-1 flex-col gap-0.5">
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
        {characters.length > 0 && (
          <p className="text-xs text-surface-300" data-ui="ModCard:characters">
            {characters.map((character, index) => (
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
        {failure != null && <FailureNote failure={failure} />}
        {!archive && (
          <p className="text-sm text-surface-400 select-none">
            {over ? m.repair_mod_drop_label() : m.repair_mod_none()}
          </p>
        )}
      </div>
      <div className="ml-auto flex shrink-0 flex-wrap justify-end gap-2">{actions}</div>
    </div>
  );
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
  const peek = usePeek(archive);
  const meta = ran ?? peek.data?.meta;
  const thumbnail = built ?? peek.data?.thumbnail;
  const chosenOut = settings.data?.["output.folder"];

  const takeMod = useTakeArchive(setArchive);
  const chooseMod = async () => {
    const picked = await chooseArchive();
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
      <ModFace
        archive={archive}
        meta={meta}
        thumbnail={thumbnail}
        characters={peek.data?.characters ?? []}
        failure={peek.isError ? peek.error : undefined}
        over={over && !running}
        actions={
          <Button size="sm" variant="outline" disabled={running} onClick={() => void chooseMod()}>
            {m.repair_mod_choose_action()}
          </Button>
        }
      />
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
