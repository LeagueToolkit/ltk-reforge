import type { ReactNode } from "react";

import { m } from "@/i18n";
import type { BuildView, HistoryBuilds, LifeView } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

export const headClass = "px-2 py-1.5 text-left text-xs font-medium text-surface-400 select-none";
export const cellClass = "px-2 py-1.5 align-top";
export const inputClass =
  "h-8 rounded-md border border-surface-600 bg-surface-950/40 px-2 font-mono text-xs text-surface-100 outline-none placeholder:text-surface-500 focus-visible:outline-2 focus-visible:outline-accent-500";

/** A life in a table's cell: its first and last builds and how many builds it spans. */
export function lifeShort(life: LifeView): string {
  return m.history_life_short({
    first: life.first.version,
    last: life.last.version,
    count: life.builds,
  });
}

/** A table on a panel, its rows the children. */
export function Table({ head, children }: { head: ReactNode; children: ReactNode }) {
  return (
    <div className="overflow-x-auto rounded-lg border border-surface-700/50">
      <table className="w-full text-sm">
        <thead className="border-b border-surface-700/50">
          <tr>{head}</tr>
        </thead>
        <tbody>{children}</tbody>
      </table>
    </div>
  );
}

/** A table's row. */
export function Row({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <tr className={twMerge("border-b border-surface-800 last:border-0", className)}>{children}</tr>
  );
}

interface BuildSelectProps {
  builds: HistoryBuilds;
  value: number | null;
  onChange: (at: number | null) => void;
  label: string;
  /** The option that picks no build, for a choice that may be none. */
  none?: string;
  className?: string;
}

/** A build of the history picked by its version, the newest first and the head marked. */
export function BuildSelect({ builds, value, onChange, label, none, className }: BuildSelectProps) {
  const newestFirst: BuildView[] = [...builds.builds].reverse();
  return (
    <select
      aria-label={label}
      value={value ?? ""}
      onChange={(event) => onChange(event.target.value === "" ? null : Number(event.target.value))}
      className={twMerge(inputClass, "bg-surface-900 pr-1", className)}
    >
      {none != null && <option value="">{none}</option>}
      {newestFirst.map((build) => (
        <option key={build.position} value={build.position}>
          {build.position === builds.head
            ? m.history_build_head({ version: build.version })
            : build.version}
        </option>
      ))}
    </select>
  );
}
