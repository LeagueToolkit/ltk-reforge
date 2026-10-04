import { InfoIcon, WarningIcon, XCircleIcon } from "@phosphor-icons/react";
import type { ReactNode } from "react";
import { match } from "ts-pattern";

import { m } from "@/i18n";
import type { Finding, Severity } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

function severityIcon(severity: Severity): ReactNode {
  return match(severity)
    .with("note", () => <InfoIcon weight="bold" className="h-4 w-4 text-surface-400" />)
    .with("warning", () => <WarningIcon weight="bold" className="h-4 w-4 text-warning-text" />)
    .with("error", () => <XCircleIcon weight="bold" className="h-4 w-4 text-danger-text" />)
    .with("fatal", () => <XCircleIcon weight="fill" className="h-4 w-4 text-danger-strong" />)
    .exhaustive();
}

/** The name of a severity, for a person. */
export function severityLabel(severity: Severity): string {
  return m[`severity.${severity}.label`]();
}

function FindingRow({ finding }: { finding: Finding }) {
  return (
    <li className="flex items-start gap-3 py-2" data-ui="FindingList:row">
      <span className="mt-0.5 shrink-0" title={severityLabel(finding.severity)}>
        {severityIcon(finding.severity)}
      </span>
      <div className="min-w-0 flex-1">
        <p className="text-sm break-words text-surface-100">{finding.message}</p>
        <p className="flex flex-wrap gap-x-2 font-mono text-xs text-surface-400">
          <span className="text-surface-300">{finding.code}</span>
          <span className="break-all">{finding.subject.name}</span>
          {finding.by && <span>{m.findings_by_label({ by: finding.by })}</span>}
          {finding.record && <span className="text-accent-300">{finding.record}</span>}
        </p>
      </div>
    </li>
  );
}

export interface FindingListProps {
  findings: Finding[];
  /** What the list says when it holds nothing. */
  empty: string;
  className?: string;
}

/** Findings in the order raised, each with its severity, code, subject and record. */
export function FindingList({ findings, empty, className }: FindingListProps) {
  if (findings.length === 0) {
    return <p className="text-sm text-surface-400 select-none">{empty}</p>;
  }
  return (
    <ul className={twMerge("divide-y divide-surface-700/50", className)} data-ui="FindingList">
      {findings.map((finding) => (
        <FindingRow key={finding.id} finding={finding} />
      ))}
    </ul>
  );
}
