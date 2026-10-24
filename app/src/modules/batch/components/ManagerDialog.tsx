import { queryOptions, useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { Button, Checkbox, Dialog, FailureNote, Spinner } from "@/components";
import { m } from "@/i18n";
import { commands, type ManagerMod } from "@/lib/bindings.gen";
import { queryFn } from "@/utils";

/** The manager's library, read when the dialog opens. */
const managerLibraryQuery = queryOptions({
  queryKey: ["manager", "library"] as const,
  queryFn: queryFn(commands.managerLibrary),
});

/**
 * Whether a mod is one the batch ports from the manager's library as it stands: a plain
 * Fantome of a champion mod, with its archive (#ADR-0106).
 */
export function isLegacyChampionMod(mod: ManagerMod): boolean {
  const champion =
    mod.tags.includes("champion-skin") || (mod.champions.length > 0 && mod.maps.length === 0);
  return mod.format === "fantome" && !mod.extended && mod.archive !== null && champion;
}

/** Why a mod is not ticked by default, in a few words, or nothing. */
function noteOf(mod: ManagerMod): string | null {
  if (mod.archive === null) {
    return m.batch_manager_note_no_archive();
  }
  if (mod.format === "modpkg") {
    return m.batch_manager_note_modpkg();
  }
  if (mod.extended) {
    return m.batch_manager_note_extended();
  }
  if (!isLegacyChampionMod(mod)) {
    return m.batch_manager_note_not_champion();
  }
  return null;
}

interface ManagerDialogProps {
  open: boolean;
  onClose: () => void;
  /** Takes the archives ticked. */
  onAdd: (paths: string[]) => void;
}

/**
 * LTK Manager's library as a source of archives (#ADR-0106): every mod with a box, the plain
 * Fantomes of champion mods ticked, the ones with no archive shown and not tickable.
 */
export function ManagerDialog({ open, onClose, onAdd }: ManagerDialogProps) {
  const library = useQuery({ ...managerLibraryQuery, enabled: open });
  const [ticked, setTicked] = useState<Set<string>>(new Set());
  const mods = library.data?.mods ?? [];

  // The default ticks are taken once the library is read, and again each time it is opened.
  useEffect(() => {
    if (open && library.data) {
      const paths = library.data.mods.filter(isLegacyChampionMod).map((mod) => mod.archive);
      setTicked(new Set(paths.filter((path): path is string => path !== null)));
    }
  }, [open, library.data]);

  const toggle = (path: string, on: boolean) =>
    setTicked((before) => {
      const next = new Set(before);
      if (on) {
        next.add(path);
      } else {
        next.delete(path);
      }
      return next;
    });
  const tickable = mods.filter((mod) => mod.archive !== null);
  const all = tickable.length > 0 && tickable.every((mod) => ticked.has(mod.archive ?? ""));
  const tickAll = (on: boolean) =>
    setTicked(
      on
        ? new Set(tickable.map((mod) => mod.archive).filter((p): p is string => p !== null))
        : new Set(),
    );
  const add = () => {
    onAdd([...ticked]);
    onClose();
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={m.batch_manager_title()}
      description={m.batch_manager_description()}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.batch_manager_cancel_action()}
          </Button>
          <Button variant="filled" disabled={ticked.size === 0} onClick={add}>
            {m.batch_manager_add_action({ count: ticked.size })}
          </Button>
        </>
      }
    >
      {library.isPending && <Spinner />}
      {library.isError && <FailureNote failure={library.error} />}
      {library.isSuccess && mods.length === 0 && (
        <p className="text-sm text-surface-400">{m.batch_manager_none()}</p>
      )}
      {mods.length > 0 && (
        <div className="flex max-h-[60vh] flex-col gap-2 overflow-y-auto" data-ui="ManagerDialog">
          <Checkbox
            checked={all}
            onCheckedChange={tickAll}
            label={m.batch_manager_all_label({ count: tickable.length })}
            className="text-surface-400"
          />
          {mods.map((mod) => {
            const note = noteOf(mod);
            const label = (
              <span className="flex min-w-0 flex-col">
                <span className="truncate text-surface-100">
                  {mod.name}
                  {mod.version && <span className="text-surface-400"> {mod.version}</span>}
                </span>
                <span className="truncate text-xs text-surface-400">
                  {[mod.champions.join(", "), note].filter(Boolean).join(" · ")}
                </span>
              </span>
            );
            if (mod.archive === null) {
              return (
                <div key={mod.slug} className="flex items-center gap-2 pl-6 opacity-60">
                  {label}
                </div>
              );
            }
            const path = mod.archive;
            return (
              <Checkbox
                key={mod.slug}
                checked={ticked.has(path)}
                onCheckedChange={(on) => toggle(path, on)}
                label={label}
              />
            );
          })}
        </div>
      )}
    </Dialog>
  );
}
