import { WarningCircleIcon } from "@phosphor-icons/react";

import { describeFailure } from "@/i18n";
import { twMerge } from "@/utils";

export interface FailureNoteProps {
  /** What a query or a mutation threw. */
  failure: unknown;
  className?: string;
}

/** A failure in place: its title, and any text from outside the app as selectable data. */
export function FailureNote({ failure, className }: FailureNoteProps) {
  const copy = describeFailure(failure);
  return (
    <div
      role="alert"
      className={twMerge("flex items-start gap-2 text-sm text-danger-text", className)}
      data-ui="FailureNote"
    >
      <WarningCircleIcon weight="bold" className="mt-0.5 h-4 w-4 shrink-0" />
      <div className="min-w-0">
        <p className="select-none">{copy.title}</p>
        {copy.detail && (
          <p className="font-mono text-xs break-words text-surface-400">{copy.detail}</p>
        )}
      </div>
    </div>
  );
}
