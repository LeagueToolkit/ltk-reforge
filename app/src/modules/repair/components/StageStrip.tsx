import { InfoIcon, WarningIcon, XCircleIcon } from "@phosphor-icons/react";
import { match } from "ts-pattern";

import { m } from "@/i18n";
import type { Finding, StageName, StageView } from "@/lib/bindings.gen";
import { countBySeverity } from "@/modules/findings";
import { twMerge } from "@/utils";

/** A stage's name for a person. */
export function stageLabel(stage: StageName): string {
  return m[`stage.${stage}.label`]();
}

function stateClass(stage: StageView): string {
  return match(stage.state)
    .with("ran", () => "border-accent-500/50 text-surface-50")
    .with("halted", () => "border-danger/60 text-danger-text")
    .with("not-run", () => "border-surface-700 text-surface-400")
    .exhaustive();
}

export interface StageStripProps {
  stages: StageView[];
  findings: Finding[];
  selected: StageName;
  onSelect: (stage: StageName) => void;
}

/** The run's stages in order, each with how far it got and what it raised. */
export function StageStrip({ stages, findings, selected, onSelect }: StageStripProps) {
  return (
    <ol className="flex flex-wrap gap-2" data-ui="StageStrip">
      {stages.map((stage) => {
        const counts = countBySeverity(findings.filter((f) => f.stage === stage.stage));
        const errors = counts.error + counts.fatal;
        return (
          <li key={stage.stage}>
            <button
              type="button"
              onClick={() => onSelect(stage.stage)}
              aria-pressed={selected === stage.stage}
              className={twMerge(
                "flex min-w-24 cursor-pointer flex-col items-start rounded-lg border bg-surface-900 px-3 py-2 text-left select-none",
                "hover:bg-surface-800 aria-pressed:bg-surface-800 aria-pressed:ring-2 aria-pressed:ring-accent-500",
                stateClass(stage),
              )}
            >
              <span className="text-sm font-medium">{stageLabel(stage.stage)}</span>
              <span className="text-xs text-surface-400">
                {m[`stage_state.${stage.state}.label`]()}
              </span>
              {stage.resumed === true && (
                <span className="text-[10px] text-surface-500">{m.run_stage_resumed_label()}</span>
              )}
              <span className="flex gap-2 text-xs">
                {errors > 0 && (
                  <span className="flex items-center gap-0.5 text-danger-text">
                    <XCircleIcon weight="bold" className="h-3 w-3" />
                    {errors}
                  </span>
                )}
                {counts.warning > 0 && (
                  <span className="flex items-center gap-0.5 text-warning-text">
                    <WarningIcon weight="bold" className="h-3 w-3" />
                    {counts.warning}
                  </span>
                )}
                {counts.note > 0 && (
                  <span className="flex items-center gap-0.5 text-surface-400">
                    <InfoIcon weight="bold" className="h-3 w-3" />
                    {counts.note}
                  </span>
                )}
              </span>
            </button>
          </li>
        );
      })}
    </ol>
  );
}
