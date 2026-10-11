import { CheckIcon, CircleNotchIcon } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import { m, stageLabel } from "@/i18n";
import type { StageName } from "@/lib/bindings.gen";
import { countBySeverity } from "@/modules/findings";
import { twMerge } from "@/utils";

import type { LiveRun } from "../state/repairStore";

/** Seconds since a moment, ticking while shown. */
function useElapsed(since: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 100);
    return () => clearInterval(timer);
  }, []);
  return (now - since) / 1000;
}

/**
 * The run under way: each stage waiting, running or done with its time, the findings raised
 * so far, and the details folded below.
 */
export function LiveProgress({ live, stages }: { live: LiveRun; stages: StageName[] }) {
  const elapsed = useElapsed(live.started);
  const counts = countBySeverity(live.findings);
  const running = stages.find((stage) => live.stages[stage]?.state === "running");
  return (
    <div
      className="flex flex-col gap-3 rounded-xl border border-accent-500/40 bg-surface-900 p-4"
      data-ui="LiveProgress"
    >
      <div className="flex items-center justify-between gap-4">
        <p className="text-sm font-medium text-surface-50 select-none">
          {m.run_live_title({ seconds: elapsed.toFixed(1) })}
        </p>
        <p className="text-xs text-surface-400 select-none">
          {m.repair_summary_counts_label({
            errors: counts.error + counts.fatal,
            warnings: counts.warning,
            notes: counts.note,
          })}
        </p>
      </div>
      <ol className="flex flex-wrap gap-2">
        {stages.map((stage) => {
          const seen = live.stages[stage];
          return (
            <li
              key={stage}
              className={twMerge(
                "flex items-center gap-1.5 rounded-md border px-2 py-1 text-xs select-none",
                !seen && "border-surface-700 text-surface-500",
                seen?.state === "running" && "border-accent-500 text-surface-50",
                (seen?.state === "ended" || seen?.state === "resumed") &&
                  "border-surface-600 text-surface-200",
              )}
            >
              {seen?.state === "running" && (
                <CircleNotchIcon weight="bold" className="h-3 w-3 animate-spin text-accent-400" />
              )}
              {(seen?.state === "ended" || seen?.state === "resumed") && (
                <CheckIcon weight="bold" className="h-3 w-3" />
              )}
              {stageLabel(stage)}
              {seen?.millis != null && (
                <span className="text-surface-400">
                  {m.run_stage_millis_label({ millis: seen.millis })}
                </span>
              )}
              {seen?.state === "resumed" && (
                <span className="text-surface-400">{m.run_stage_resumed_label()}</span>
              )}
            </li>
          );
        })}
      </ol>
      {live.stopping && running && (
        <p className="text-xs text-warning-text select-none" data-ui="LiveProgress:stopping">
          {m.run_live_stopping_label({ stage: stageLabel(running) })}
        </p>
      )}
    </div>
  );
}
