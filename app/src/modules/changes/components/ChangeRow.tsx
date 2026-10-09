import { ArrowRightIcon } from "@phosphor-icons/react";
import { match } from "ts-pattern";

import { m } from "@/i18n";
import type { ChangeView, OutcomeView, ValueView } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

import { countOutcomes, movedParts, OUTCOMES, type ChangeItem } from "../model";

/** Each outcome's colour: a converted value is good, a removed one plain, a lost one a warning. */
const outcomeTone: Record<OutcomeView, string> = {
  changed: "bg-success",
  removed: "bg-surface-500",
  lost: "bg-warning",
  unconvertible: "bg-warning",
};

/** The dot of an outcome's colour, named by its description. */
export function OutcomeDot({ outcome, className }: { outcome: OutcomeView; className?: string }) {
  return (
    <span
      className={twMerge(
        "inline-block h-1.5 w-1.5 shrink-0 rounded-full",
        outcomeTone[outcome],
        className,
      )}
      title={m[`changes_outcome.${outcome}.description`]()}
    />
  );
}

/** How many changes there are of each outcome that has any, each beside its dot. */
export function OutcomeCounts({ items }: { items: ChangeItem[] }) {
  const counts = countOutcomes(items);
  return (
    <span className="inline-flex flex-wrap items-center gap-x-3 text-xs text-surface-400">
      {OUTCOMES.filter((outcome) => counts[outcome] > 0).map((outcome) => (
        <span key={outcome} className="inline-flex items-center gap-1.5">
          <OutcomeDot outcome={outcome} />
          {counts[outcome]} {m[`changes_outcome.${outcome}.label`]()}
        </span>
      ))}
    </span>
  );
}

function Value({ value, className }: { value: ValueView; className?: string }) {
  return (
    <span className="inline-flex min-w-0 items-baseline gap-1">
      <span className="rounded bg-surface-800 px-1 font-sans text-[10px] text-surface-400">
        {value.kind}
      </span>
      <span className={twMerge("break-all", className)}>{value.text}</span>
    </span>
  );
}

function After({ change }: { change: ChangeView }) {
  return match(change.after)
    .with({ as: "value" }, (after) => <Value value={after} className="text-success-text" />)
    .with({ as: "moved" }, ({ to }) => (
      <span className="min-w-0 break-all">
        <span className="font-sans text-surface-400">{m.changes_after_moved()} </span>
        {movedParts(change.path, to).map((part, at) => (
          <span key={at}>
            {at > 0 && "."}
            <span className={part.added ? "text-accent-300" : "text-surface-300"}>{part.text}</span>
          </span>
        ))}
      </span>
    ))
    .with({ as: "removed" }, () => (
      <span className="font-sans text-surface-400">{m.changes_after_removed()}</span>
    ))
    .with({ as: "lost" }, () => (
      <span className="font-sans text-warning-text">{m.changes_after_lost()}</span>
    ))
    .with({ as: "unconvertible" }, ({ wants }) => (
      <span className="font-sans text-warning-text">
        {m.changes_after_unconvertible({ wants })}
      </span>
    ))
    .exhaustive();
}

export interface ChangeRowProps {
  change: ChangeView;
  /** The fixup that made it, for a list that mixes fixups. */
  fixup?: string;
}

/**
 * One value a fixup changed: where it was, what it was and what is there now. The path's
 * class and field are its title.
 */
export function ChangeRow({ change, fixup }: ChangeRowProps) {
  const gone = change.outcome === "removed" || change.outcome === "lost";
  return (
    <li className="flex gap-2 py-1.5" data-ui="ChangeRow">
      <OutcomeDot outcome={change.outcome} className="mt-1.5" />
      <div className="min-w-0 flex-1 font-mono text-xs">
        <p className="break-all text-surface-100" title={change.field}>
          {change.path}
        </p>
        <p className="flex flex-wrap items-baseline gap-x-1.5 text-surface-300">
          {change.before ? (
            <Value
              value={change.before}
              className={gone ? "text-surface-400 line-through" : undefined}
            />
          ) : (
            <span className="font-sans text-surface-400">{m.changes_added_label()}</span>
          )}
          <ArrowRightIcon className="h-3 w-3 shrink-0 self-center text-surface-500" />
          <After change={change} />
        </p>
        {(fixup !== undefined || !change.reached) && (
          <p className="flex flex-wrap gap-x-2 font-sans text-surface-500">
            {fixup && <span>{fixup}</span>}
            {!change.reached && <span>{m.changes_unreached_label()}</span>}
          </p>
        )}
      </div>
    </li>
  );
}
