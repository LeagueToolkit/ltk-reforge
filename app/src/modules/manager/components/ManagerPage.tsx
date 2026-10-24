import {
  ArrowsClockwiseIcon,
  CopyIcon,
  MagnifyingGlassIcon,
  QueueIcon,
  ScanIcon,
  StopIcon,
} from "@phosphor-icons/react";
import { useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { Button, Checkbox, FailureNote, Spinner, Tooltip } from "@/components";
import { m } from "@/i18n";
import type { BatchRow, ManagerMod } from "@/lib/bindings.gen";
import { useBatchStore } from "@/modules/batch";
import { useJobRunning } from "@/modules/repair";
import { copyText, twMerge } from "@/utils";

import { useManagerLibrary } from "../api/manager";
import { useScan, useScanEvents, useScanRunning, useScanStore, useStopScan } from "../api/scan";
import { isLegacyChampionMod, type ManagerNote, noteOf } from "../legacy";
import {
  hasProblem,
  isFixable,
  isPaste,
  movesToPremium,
  originText,
  reportText,
  slotText,
  stateText,
  unitMovesToPremium,
} from "../scan";
import { useTickStore } from "../state/tickStore";

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

/**
 * What the scan comes to for a mod, a line per unit in the person's words; the evidence, where
 * the unit's work comes from and where a port lands it, is the line's tooltip.
 */
function OriginCell({ row, running }: { row: BatchRow | undefined; running: boolean }) {
  if (running) {
    return (
      <span className="flex items-center gap-1.5 text-surface-400">
        <Spinner size="sm" />
        {m.manager_scan_running()}
      </span>
    );
  }
  if (!row) {
    return null;
  }
  if (row.outcome.outcome !== "scanned" || row.units.length === 0) {
    return <span className="text-surface-500">{m.manager_scan_no_verdict()}</span>;
  }
  return (
    <span className="flex flex-col gap-0.5">
      {row.units.map((unit, index) => (
        // A paste that stays at the base is a warning; one a port moves to a premium skin is
        // what the person came to find, so it stands out from the rest.
        <Tooltip
          key={index}
          content={m.manager_origin_line({
            character: unit.character,
            origin: originText(unit),
            landing: slotText(unit.chosen),
          })}
        >
          <span
            className={twMerge(
              "cursor-help self-start",
              unitMovesToPremium(unit)
                ? "text-danger-text"
                : isPaste(unit)
                  ? "text-warning-text"
                  : "text-surface-300",
            )}
          >
            {row.units.length > 1 ? `${unit.character}: ${stateText(unit)}` : stateText(unit)}
          </span>
        </Tooltip>
      ))}
    </span>
  );
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
  useScanEvents();
  const library = useManagerLibrary();
  const addArchives = useBatchStore((state) => state.addArchives);
  const navigate = useNavigate();
  const scan = useScan();
  const stopScan = useStopScan();
  const scanning = useScanRunning();
  const busy = useJobRunning();
  const scanRows = useScanStore((state) => state.rows);
  const scanAt = useScanStore((state) => state.at);
  const scanArchives = useScanStore((state) => state.archives);
  const underScan = new Set((scanAt?.running ?? []).map((index) => scanArchives[index]));
  const stopping = useScanStore((state) => state.stopping);
  const ticked = useTickStore((state) => state.ticked);
  const tickedAt = useTickStore((state) => state.readAt);
  const setTickedAt = useTickStore((state) => state.setTicked);
  const setTicked = (next: Set<string> | ((before: Set<string>) => Set<string>)) =>
    setTickedAt(typeof next === "function" ? next(useTickStore.getState().ticked) : next);
  const [filter, setFilter] = useState("");
  const [pastesOnly, setPastesOnly] = useState(false);
  const mods = library.data?.mods ?? [];
  // The filter narrows what is shown, and "all" ticks what is shown: a few are picked by
  // typing a name, ticking all, then another.
  const shown = mods.filter((mod) => {
    if (!matchesFilter(mod, filter)) {
      return false;
    }
    const row = mod.archive === null ? undefined : scanRows[mod.archive];
    return !pastesOnly || (row !== undefined && hasProblem(row));
  });
  const tickable = shown.filter((mod) => mod.archive !== null);
  const scanned = Object.values(scanRows);
  const counts = {
    scanned: scanned.length,
    fixable: scanned.filter(isFixable).length,
    moved: scanned.filter(movesToPremium).length,
  };

  // The default ticks are taken each time the library is read, and kept across pages: a
  // return to the page with the library as it was read keeps what was ticked.
  const readAt = library.dataUpdatedAt;
  useEffect(() => {
    if (library.data && readAt !== tickedAt) {
      setTickedAt(new Set(archivesOf(library.data.mods.filter(isLegacyChampionMod))), readAt);
    }
  }, [library.data, readAt, tickedAt, setTickedAt]);

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
  // The scanned mods a port fixes while they stay at the base (#ADR-0107): the ones that
  // move to a premium skin are a list to report, not to port.
  const fixable = Object.entries(scanRows)
    .filter(([, row]) => isFixable(row))
    .map(([path]) => path);
  const tickFixable = () => setTicked(new Set(fixable));
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
            size="sm"
            variant="outline"
            disabled={fixable.length === 0}
            onClick={tickFixable}
            data-ui="ManagerPage:tickFixable"
          >
            {m.manager_tick_fixable_action({ count: fixable.length })}
          </Button>
          <Button
            size="sm"
            variant="outline"
            left={<CopyIcon weight="bold" />}
            disabled={counts.moved === 0}
            onClick={() => copyText(reportText(scanned))}
            data-ui="ManagerPage:report"
          >
            {m.manager_report_action({ count: counts.moved })}
          </Button>
          {scanning ? (
            <Button
              size="sm"
              variant="outline"
              left={<StopIcon weight="bold" />}
              loading={stopping}
              onClick={() => stopScan.mutate()}
            >
              {m.manager_scan_stop_action()}
            </Button>
          ) : (
            <Button
              size="sm"
              variant="outline"
              left={<ScanIcon weight="bold" />}
              disabled={ticked.size === 0 || busy}
              onClick={() => scan.mutate([...ticked])}
              data-ui="ManagerPage:scan"
            >
              {m.manager_scan_action({ count: ticked.size })}
            </Button>
          )}
          <Button
            variant="filled"
            left={<QueueIcon weight="bold" />}
            disabled={ticked.size === 0 || busy}
            onClick={toBatch}
          >
            {m.manager_to_batch_action({ count: ticked.size })}
          </Button>
        </div>
      </div>
      {scan.isError && <FailureNote failure={scan.error} />}
      {scanAt && (
        <div className="flex flex-col gap-2" data-ui="ManagerPage:progress">
          <p className="truncate text-sm text-surface-200 select-none">
            {m.manager_scan_progress({
              done: scanAt.done,
              total: scanAt.total,
              running: scanAt.running.length,
            })}
          </p>
          <div className="h-1.5 overflow-hidden rounded-full bg-surface-800">
            <div
              className="h-full bg-accent-500 transition-[width]"
              style={{ width: `${(scanAt.done / scanAt.total) * 100}%` }}
            />
          </div>
        </div>
      )}
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
          <Checkbox
            checked={pastesOnly}
            onCheckedChange={setPastesOnly}
            label={m.manager_pastes_only_label()}
            className="text-surface-300"
          />
          <p className="text-xs text-surface-400 select-none">
            {m.manager_ticked_label({
              ticked: ticked.size,
              shown: shown.length,
              total: mods.length,
            })}
            {counts.scanned > 0 && ` · ${m.manager_scan_counts(counts)}`}
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
                <th className={headClass}>{m.manager_table_origin_label()}</th>
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
                    <td className={twMerge(cellClass, "text-xs")}>
                      <OriginCell
                        row={path === null ? undefined : scanRows[path]}
                        running={path !== null && underScan.has(path)}
                      />
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
