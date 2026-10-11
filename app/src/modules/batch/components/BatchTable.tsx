import { Link } from "@tanstack/react-router";
import { match } from "ts-pattern";

import { Tooltip } from "@/components";
import { m } from "@/i18n";
import type { BatchRow, Outcome, SeverityCounts } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

import { type OutcomeTone, outcomeTone } from "../outcomes";

const toneClass: Record<OutcomeTone, string> = {
  success: "border-success/40 bg-success/10 text-success-text",
  warning: "border-warning/40 bg-warning/10 text-warning-text",
  danger: "border-danger/40 bg-danger/10 text-danger-text",
  muted: "border-surface-700 bg-surface-900 text-surface-400",
};

function outcomeLabel(outcome: Outcome): string {
  return match(outcome)
    .with({ outcome: "written" }, () => m.batch_outcome_written())
    .with({ outcome: "refused" }, () => m.batch_outcome_refused())
    .with({ outcome: "halted" }, () => m.batch_outcome_halted())
    .with({ outcome: "cancelled" }, ({ stage }) =>
      m.batch_outcome_cancelled({ stage: m[`stage.${stage}.label`]() }),
    )
    .with({ outcome: "fault" }, () => m.batch_outcome_fault())
    .with({ outcome: "unreadable" }, () => m.batch_outcome_unreadable())
    .exhaustive();
}

/** An outcome as a badge in its tone, with the halting code or the fault beside it. */
function OutcomeBadge({ outcome }: { outcome: Outcome }) {
  const badge = (
    <span
      className={twMerge(
        "inline-flex rounded-md border px-1.5 py-0.5 text-xs whitespace-nowrap select-none",
        toneClass[outcomeTone(outcome)],
      )}
    >
      {outcomeLabel(outcome)}
    </span>
  );
  return match(outcome)
    .with({ outcome: "halted" }, ({ code }) => (
      <span className="flex flex-col items-start gap-0.5">
        {badge}
        <span className="font-mono text-xs text-surface-400">{code}</span>
      </span>
    ))
    .with({ outcome: "fault" }, ({ detail }) => <Tooltip content={detail}>{badge}</Tooltip>)
    .otherwise(() => badge);
}

/** A run's findings as three small numbers, each in its severity's colour. */
function Counts({ counts }: { counts: SeverityCounts }) {
  const shown = [
    {
      count: counts.fatal + counts.error,
      title: m.batch_count_error_title(),
      className: "text-danger-text",
    },
    { count: counts.warning, title: m.batch_count_warning_title(), className: "text-warning-text" },
    { count: counts.note, title: m.batch_count_note_title(), className: "text-surface-400" },
  ];
  return (
    <span className="flex gap-2 font-mono text-xs tabular-nums">
      {shown.map(({ count, title, className }) => (
        <span
          key={title}
          title={title}
          className={twMerge(count === 0 ? "text-surface-600" : className)}
        >
          {count}
        </span>
      ))}
    </span>
  );
}

const headClass = "px-2 py-1.5 text-left text-xs font-medium text-surface-400 select-none";
// Cells of text, counts and a badge in different sizes line up on their first line's baseline.
const cellClass = "px-2 py-1.5 align-baseline";

/** The outcome of each archive of a batch, a row per run, in the order they ran. */
export function BatchTable({ rows }: { rows: readonly BatchRow[] }) {
  return (
    <div className="overflow-x-auto rounded-xl border border-surface-700/50 bg-surface-900/95">
      <table className="w-full text-sm" data-ui="BatchTable">
        <thead className="border-b border-surface-700/50">
          <tr>
            <th className={headClass}>{m.batch_table_file_label()}</th>
            <th className={headClass}>{m.batch_table_mod_label()}</th>
            <th className={headClass}>{m.batch_table_outcome_label()}</th>
            <th className={headClass}>{m.batch_table_findings_label()}</th>
            <th className={twMerge(headClass, "text-right")}>{m.batch_table_time_label()}</th>
            <th className={headClass} />
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.index} className="border-b border-surface-800 last:border-0">
              <td className={twMerge(cellClass, "break-all text-surface-100")}>{row.archive}</td>
              <td className={twMerge(cellClass, "break-words text-surface-300")}>
                {row.name ?? ""}
              </td>
              <td className={cellClass}>
                <OutcomeBadge outcome={row.outcome} />
              </td>
              <td className={cellClass}>
                <Counts counts={row.counts} />
              </td>
              <td
                className={twMerge(
                  cellClass,
                  "text-right text-xs whitespace-nowrap text-surface-400 tabular-nums",
                )}
              >
                {m.batch_time_label({ seconds: (row.millis / 1000).toFixed(1) })}
              </td>
              <td className={twMerge(cellClass, "text-right")}>
                {row.saved != null && (
                  <Link
                    to="/runs"
                    search={{ run: row.saved }}
                    className="text-xs text-accent-300 underline-offset-2 hover:underline"
                  >
                    {m.batch_open_action()}
                  </Link>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
