import { InfoIcon, WarningIcon, XCircleIcon } from "@phosphor-icons/react";
import type { ReactNode } from "react";
import { match } from "ts-pattern";

import { Disclosure } from "@/components";
import { m } from "@/i18n";
import type { Finding, Severity } from "@/lib/bindings.gen";
import { RecordLink } from "@/modules/records";
import { twMerge } from "@/utils";

import { typeFinding } from "../types";
import { DataTree, hasFields, type IdNames, subjectName } from "./DataTree";
import { TypeDetail } from "./TypeDetail";

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

interface FindingHeadProps {
  finding: Finding;
  folds: boolean;
  names: IdNames | undefined;
}

function FindingHead({ finding, folds, names }: FindingHeadProps) {
  return (
    <span className="flex items-start gap-3">
      <span className="mt-0.5 shrink-0" title={severityLabel(finding.severity)}>
        {severityIcon(finding.severity)}
      </span>
      <span className="min-w-0 flex-1">
        <span
          className={twMerge(
            "block text-sm break-words text-surface-100",
            folds && "line-clamp-2 group-data-[panel-open]:line-clamp-none",
          )}
        >
          {finding.message}
        </span>
        <span className="flex flex-wrap gap-x-2 font-mono text-xs text-surface-400">
          <span className="text-surface-300">{finding.code}</span>
          <span className="break-all">{subjectName(finding.subject, names)}</span>
          {finding.by && <span>{m.findings_by_label({ by: finding.by })}</span>}
          {finding.record && <RecordLink code={finding.record} />}
        </span>
      </span>
    </span>
  );
}

/** A finding's fields: a type check's rows as their own detail, another's as their tree. */
function FieldsOf({ finding, names }: { finding: Finding; names: IdNames | undefined }) {
  const found = typeFinding(finding);
  return found ? <TypeDetail found={found} /> : <DataTree value={finding.data} names={names} />;
}

interface FindingRowProps {
  finding: Finding;
  detail: ReactNode;
  names: IdNames | undefined;
}

function FindingRow({ finding, detail, names }: FindingRowProps) {
  const opens = detail != null || hasFields(finding.data);
  if (!opens) {
    return (
      <li className="py-2 pl-5" data-ui="FindingList:row">
        <FindingHead finding={finding} folds={false} names={names} />
      </li>
    );
  }
  return (
    <li data-ui="FindingList:row">
      <Disclosure
        summary={<FindingHead finding={finding} folds names={names} />}
        summaryClassName="py-2"
        panelClassName="pb-3 pl-12"
      >
        {detail ?? <FieldsOf finding={finding} names={names} />}
      </Disclosure>
    </li>
  );
}

export interface FindingListProps {
  findings: Finding[];
  /** What the list says when it holds nothing. */
  empty: string;
  className?: string;
  /**
   * What a finding opens to in place of its fields, such as the changes a fixup's finding
   * counts. Null for a finding that opens to its fields, when it has any.
   */
  detailOf?: (finding: Finding) => ReactNode;
  /** A name for each id a finding's fields can hold. */
  names?: IdNames;
}

/**
 * Findings in the order given, each with its severity, code, subject and record, and each
 * opening to its details: what the caller draws for it, else its fields.
 */
export function FindingList({ findings, empty, className, detailOf, names }: FindingListProps) {
  if (findings.length === 0) {
    return <p className="text-sm text-surface-400 select-none">{empty}</p>;
  }
  return (
    <ul className={twMerge("divide-y divide-surface-700/50", className)} data-ui="FindingList">
      {findings.map((finding) => (
        <FindingRow key={finding.id} finding={finding} detail={detailOf?.(finding)} names={names} />
      ))}
    </ul>
  );
}
