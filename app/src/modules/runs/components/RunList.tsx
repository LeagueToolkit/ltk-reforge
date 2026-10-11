import { MagnifyingGlassIcon, PackageIcon, PushPinIcon } from "@phosphor-icons/react";
import { useState } from "react";
import { match } from "ts-pattern";

import { Tooltip } from "@/components";
import { m } from "@/i18n";
import type { RunKind, RunRow } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

import { type Ago, ago, localTime, storeTime } from "../time";

/** How long ago, for a person. */
export function agoLabel(when: Ago): string {
  return match(when)
    .with({ unit: "now" }, () => m.runs_ago_now())
    .with({ unit: "minutes" }, ({ count }) => m.runs_ago_minutes({ count }))
    .with({ unit: "hours" }, ({ count }) => m.runs_ago_hours({ count }))
    .with({ unit: "yesterday" }, () => m.runs_ago_yesterday())
    .with({ unit: "days" }, ({ count }) => m.runs_ago_days({ count }))
    .with({ unit: "date" }, ({ date }) => date)
    .exhaustive();
}

/** A run's kind, in a word. */
export function KindBadge({ kind }: { kind: RunKind }) {
  return (
    <span
      className="shrink-0 rounded-full bg-surface-veil px-2 py-0.5 text-[11px] font-medium text-surface-200 select-none"
      data-ui="KindBadge"
    >
      {m[`runs_kind.${kind}.label`]()}
    </span>
  );
}

/** A run's name for a person: the mod's, else its archive's file name. */
export function runName(row: RunRow): string {
  return row.name ?? row.file_name;
}

/** Whether a run's name or author holds the words typed, in any case. */
export function matches(row: RunRow, filter: string): boolean {
  const words = filter.trim().toLowerCase();
  if (!words) {
    return true;
  }
  return [row.name, row.author, row.file_name].some((text) => text?.toLowerCase().includes(words));
}

/** The button a run is picked by, in the list and wherever a run is chosen. */
export const runButtonClass =
  "flex w-full cursor-pointer flex-col gap-1 rounded-lg px-3 py-2 text-left select-none hover:bg-surface-veil";

/** When the store wrote a run, as a time ago with the full time on hover. */
export function RunWhen({ madeAt, now }: { madeAt: string; now: Date }) {
  const made = storeTime(madeAt);
  return (
    <Tooltip content={localTime(made)}>
      <span className="shrink-0">{agoLabel(ago(made, now))}</span>
    </Tooltip>
  );
}

/** A run as the list draws it: its name and marks, then its kind, author and when. */
export function RunLine({ row, now }: { row: RunRow; now: Date }) {
  return (
    <>
      <span className="flex min-w-0 items-center gap-2">
        <span className="min-w-0 flex-1 truncate text-sm text-surface-50">{runName(row)}</span>
        {row.kept && (
          <Tooltip content={m.runs_kept_label()}>
            <PushPinIcon weight="fill" className="h-3.5 w-3.5 shrink-0 text-accent-300" />
          </Tooltip>
        )}
        {row.package && (
          <Tooltip content={m.runs_package_label()}>
            <PackageIcon weight="bold" className="h-3.5 w-3.5 shrink-0 text-surface-300" />
          </Tooltip>
        )}
      </span>
      <span className="flex min-w-0 items-center gap-2 text-xs text-surface-400">
        <KindBadge kind={row.kind} />
        {!row.archive_ingested && (
          <Tooltip content={m.runs_ingest_cleared_hint()}>
            <span
              className="shrink-0 rounded-full border border-surface-600 px-2 py-0.5 text-[11px] text-surface-300 select-none"
              data-ui="RunList:cleared"
            >
              {m.runs_ingest_cleared_label()}
            </span>
          </Tooltip>
        )}
        {row.author && (
          <span className="min-w-0 truncate">{m.runs_by_label({ author: row.author })}</span>
        )}
        <span className="ml-auto shrink-0">
          <RunWhen madeAt={row.made_at} now={now} />
        </span>
      </span>
    </>
  );
}

interface RunListProps {
  runs: RunRow[];
  selected: number | null;
  onSelect: (id: number) => void;
  /** The time the ages are counted to. */
  now: Date;
  className?: string;
}

/** The runs kept, newest first, filtered by name and author, one marked as selected. */
export function RunList({ runs, selected, onSelect, now, className }: RunListProps) {
  const [filter, setFilter] = useState("");
  const shown = runs.filter((row) => matches(row, filter));
  return (
    <div className={twMerge("flex flex-col gap-3", className)} data-ui="RunList">
      <label className="flex items-center gap-2 rounded-md border border-surface-600 bg-surface-950/40 px-2 focus-within:outline-2 focus-within:outline-accent-500">
        <MagnifyingGlassIcon className="h-3.5 w-3.5 shrink-0 text-surface-400" />
        <input
          type="search"
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          placeholder={m.runs_filter_placeholder()}
          aria-label={m.runs_filter_label()}
          className="h-8 min-w-0 flex-1 bg-transparent text-xs text-surface-100 outline-none placeholder:text-surface-500"
        />
      </label>
      {shown.length === 0 && (
        <p className="text-sm text-surface-400 select-none">{m.runs_none_match()}</p>
      )}
      <ul className="flex flex-col gap-1" aria-label={m.runs_list_label()}>
        {shown.map((row) => (
          <li key={row.id}>
            <button
              type="button"
              aria-pressed={row.id === selected}
              onClick={() => onSelect(row.id)}
              className={twMerge(
                runButtonClass,
                "aria-pressed:bg-accent-500/10 aria-pressed:ring-1 aria-pressed:ring-accent-500/35 aria-pressed:ring-inset",
              )}
              data-ui="RunList:row"
            >
              <RunLine row={row} now={now} />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
