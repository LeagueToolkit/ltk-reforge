import { ArrowsClockwiseIcon, MagnifyingGlassIcon, QueueIcon } from "@phosphor-icons/react";
import { useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { Button, Checkbox, FailureNote, Spinner } from "@/components";
import { m } from "@/i18n";
import type { ManagerMod } from "@/lib/bindings.gen";
import { useBatchStore } from "@/modules/batch";
import { twMerge } from "@/utils";

import { useManagerLibrary } from "../api/manager";
import { isLegacyChampionMod, type ManagerNote, noteOf } from "../legacy";

const headClass = "px-3 py-2 text-left text-xs font-medium text-surface-400 select-none";
const cellClass = "px-3 py-2 align-top";

function noteLabel(note: ManagerNote): string {
  switch (note) {
    case "no-archive":
      return m.manager_note_no_archive();
    case "modpkg":
      return m.manager_note_modpkg();
    case "extended":
      return m.manager_note_extended();
    case "not-champion":
      return m.manager_note_not_champion();
  }
}

/** The archives of the mods that have one. */
function archivesOf(mods: readonly ManagerMod[]): string[] {
  return mods.map((mod) => mod.archive).filter((path): path is string => path !== null);
}

/** Whether a mod's name, slug, version or a champion holds the text, whatever the case. */
export function matchesFilter(mod: ManagerMod, text: string): boolean {
  const needle = text.trim().toLowerCase();
  if (needle === "") {
    return true;
  }
  return [mod.name, mod.slug, mod.version ?? "", ...mod.champions].some((field) =>
    field.toLowerCase().includes(needle),
  );
}

/**
 * LTK Manager's library at the window's width (#ADR-0106): every mod with a box, the plain
 * Fantomes of champion mods ticked, and the ticked archives handed to the Batch page.
 */
export function ManagerPage() {
  const library = useManagerLibrary();
  const addArchives = useBatchStore((state) => state.addArchives);
  const navigate = useNavigate();
  const [ticked, setTicked] = useState<Set<string>>(new Set());
  const [filter, setFilter] = useState("");
  const mods = library.data?.mods ?? [];
  // The filter narrows what is shown, and "all" ticks what is shown: a few are picked by
  // typing a name, ticking all, then another.
  const shown = mods.filter((mod) => matchesFilter(mod, filter));
  const tickable = shown.filter((mod) => mod.archive !== null);

  // The default ticks are taken each time the library is read.
  useEffect(() => {
    if (library.data) {
      setTicked(new Set(archivesOf(library.data.mods.filter(isLegacyChampionMod))));
    }
  }, [library.data]);

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
  const all = tickable.length > 0 && tickable.every((mod) => ticked.has(mod.archive ?? ""));
  const tickShown = (on: boolean) =>
    setTicked((before) => {
      const next = new Set(before);
      for (const path of archivesOf(tickable)) {
        if (on) {
          next.add(path);
        } else {
          next.delete(path);
        }
      }
      return next;
    });
  const tickLegacy = () => setTicked(new Set(archivesOf(mods.filter(isLegacyChampionMod))));
  const toBatch = () => {
    addArchives([...ticked]);
    void navigate({ to: "/batch" });
  };

  return (
    <div className="flex flex-col gap-6" data-ui="ManagerPage">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 className="font-display text-xl font-semibold text-surface-50">
            {m.manager_page_title()}
          </h1>
          <p className="text-sm text-surface-400 select-none">{m.manager_page_description()}</p>
          {library.data && (
            <p className="font-mono text-xs break-all text-surface-500">{library.data.folder}</p>
          )}
        </div>
        <div className="flex shrink-0 flex-wrap gap-2">
          <Button
            size="sm"
            variant="outline"
            left={<ArrowsClockwiseIcon weight="bold" />}
            loading={library.isFetching}
            onClick={() => void library.refetch()}
          >
            {m.manager_refresh_action()}
          </Button>
          <Button size="sm" variant="outline" disabled={mods.length === 0} onClick={tickLegacy}>
            {m.manager_tick_legacy_action()}
          </Button>
          <Button
            variant="filled"
            left={<QueueIcon weight="bold" />}
            disabled={ticked.size === 0}
            onClick={toBatch}
          >
            {m.manager_to_batch_action({ count: ticked.size })}
          </Button>
        </div>
      </div>
      {library.isPending && <Spinner />}
      {library.isError && <FailureNote failure={library.error} />}
      {library.isSuccess && mods.length === 0 && (
        <p className="text-sm text-surface-400">{m.manager_none()}</p>
      )}
      {mods.length > 0 && (
        <div className="flex flex-wrap items-center gap-3">
          <label className="flex max-w-md min-w-48 flex-1 items-center gap-2 rounded-md border border-surface-600 bg-surface-950/40 px-2 focus-within:outline-2 focus-within:outline-accent-500">
            <MagnifyingGlassIcon className="h-3.5 w-3.5 shrink-0 text-surface-400" />
            <input
              type="search"
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
              placeholder={m.manager_filter_placeholder()}
              aria-label={m.manager_filter_label()}
              className="h-8 min-w-0 flex-1 bg-transparent text-sm text-surface-100 outline-none placeholder:text-surface-500"
            />
          </label>
          <p className="text-xs text-surface-400 select-none">
            {m.manager_ticked_label({
              ticked: ticked.size,
              shown: shown.length,
              total: mods.length,
            })}
          </p>
        </div>
      )}
      {shown.length > 0 && (
        <div className="overflow-x-auto rounded-xl border border-surface-700/50 bg-surface-900/95">
          <table className="w-full text-sm" data-ui="ManagerTable">
            <thead className="border-b border-surface-700/50">
              <tr>
                <th className={headClass}>
                  <Checkbox
                    checked={all}
                    onCheckedChange={tickShown}
                    label={m.manager_table_all_label({ count: tickable.length })}
                  />
                </th>
                <th className={headClass}>{m.manager_table_version_label()}</th>
                <th className={headClass}>{m.manager_table_champions_label()}</th>
                <th className={headClass}>{m.manager_table_note_label()}</th>
                <th className={headClass}>{m.manager_table_installed_label()}</th>
              </tr>
            </thead>
            <tbody>
              {shown.map((mod) => {
                const note = noteOf(mod);
                const path = mod.archive;
                return (
                  <tr
                    key={mod.slug}
                    className={twMerge(
                      "border-b border-surface-800 last:border-0",
                      path === null && "opacity-60",
                    )}
                  >
                    <td className={twMerge(cellClass, "break-words text-surface-100")}>
                      {path === null ? (
                        <span className="pl-6">{mod.name}</span>
                      ) : (
                        <Checkbox
                          checked={ticked.has(path)}
                          onCheckedChange={(on) => toggle(path, on)}
                          label={mod.name}
                        />
                      )}
                    </td>
                    <td className={twMerge(cellClass, "whitespace-nowrap text-surface-300")}>
                      {mod.version ?? ""}
                    </td>
                    <td className={twMerge(cellClass, "break-words text-surface-300")}>
                      {mod.champions.join(", ")}
                    </td>
                    <td className={twMerge(cellClass, "text-xs text-surface-400")}>
                      {note ? noteLabel(note) : m.manager_note_legacy()}
                    </td>
                    <td
                      className={twMerge(cellClass, "text-xs whitespace-nowrap text-surface-400")}
                    >
                      {mod.installed_at.slice(0, 10)}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
