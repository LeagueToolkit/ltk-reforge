import { MagnifyingGlassIcon } from "@phosphor-icons/react";
import { useMemo, useState } from "react";

import { Disclosure, SectionCard, SegmentedControl } from "@/components";
import { m } from "@/i18n";
import type { Finding, FixupChanges, ObjectView, OutcomeView } from "@/lib/bindings.gen";
import { useFixupCatalog } from "@/modules/fixups";
import { twMerge } from "@/utils";

import {
  type ChangeItem,
  countOutcomes,
  fieldCounts,
  groupBy,
  itemsOf,
  matchesText,
  OUTCOMES,
  outcomeOfCode,
} from "../model";
import { OutcomeCounts, OutcomeDot } from "./ChangeRow";
import { ObjectGroups, Paged } from "./ObjectGroups";

type Grouping = "fixup" | "object";

interface FixupGroupProps {
  fixup: string;
  title: string | undefined;
  /** The fixup's own findings: their codes and records are shown with its changes. */
  findings: Finding[];
  items: ChangeItem[];
  onField: (field: string) => void;
}

function FixupGroup({ fixup, title, findings, items, onField }: FixupGroupProps) {
  const records = [...new Set(findings.flatMap((finding) => finding.record ?? []))];
  return (
    <Disclosure
      summary={
        <span className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
          <span className="min-w-0">
            <span className="text-sm text-surface-100">{title ?? fixup}</span>{" "}
            <span className="font-mono text-xs text-surface-400">{fixup}</span>
          </span>
          <OutcomeCounts items={items} />
        </span>
      }
      summaryClassName="py-2"
      panelClassName="flex flex-col gap-3 pb-3 pl-5"
      className="border-b border-surface-700/50 last:border-b-0"
    >
      <p className="flex flex-wrap gap-x-3 gap-y-1 font-mono text-xs">
        {findings.map((finding) => (
          <span key={finding.id} className="text-surface-300" title={finding.message}>
            {finding.code}
          </span>
        ))}
        {records.map((record) => (
          <span key={record} className="text-accent-300">
            {record}
          </span>
        ))}
      </p>
      <div className="flex flex-wrap gap-1.5">
        {fieldCounts(items).map(({ field, count }) => (
          <button
            key={field}
            type="button"
            onClick={() => onField(field)}
            title={m.changes_field_filter_label({ field })}
            className="cursor-pointer rounded-md border border-surface-700 px-1.5 py-0.5 font-mono text-[11px] text-surface-200 hover:border-accent-500/60 hover:text-surface-50"
          >
            {field} <span className="text-surface-500">{count}</span>
          </button>
        ))}
      </div>
      <ObjectGroups items={items} />
    </Disclosure>
  );
}

export interface ChangesExplorerProps {
  changes: FixupChanges[];
  objects: ObjectView[];
  /** The run's findings: a fixup's `fixup.*` findings are shown with its changes. */
  findings: Finding[];
}

/**
 * Every value the fixups changed, by fixup or by object, each opening to the values before and
 * after. A filter finds a change by its object, field, path or value, and an outcome is shown
 * or hidden.
 */
export function ChangesExplorer({ changes, objects, findings }: ChangesExplorerProps) {
  const catalog = useFixupCatalog();
  const titles = new Map(catalog.data?.map((entry) => [entry.id, entry.title]));
  const [filter, setFilter] = useState("");
  const [hidden, setHidden] = useState<OutcomeView[]>([]);
  const [grouping, setGrouping] = useState<Grouping>("fixup");
  const all = useMemo(() => itemsOf(changes, objects), [changes, objects]);
  const shown = useMemo(
    () => all.filter((item) => !hidden.includes(item.change.outcome) && matchesText(item, filter)),
    [all, hidden, filter],
  );
  const totals = countOutcomes(all);
  const groupings = [
    { value: "fixup" as const, label: m.changes_group_fixup_label() },
    { value: "object" as const, label: m.changes_group_object_label() },
  ];
  const fixupFindings = (fixup: string) =>
    findings.filter((finding) => finding.by === fixup && outcomeOfCode(finding.code) !== null);
  const toggle = (outcome: OutcomeView) =>
    setHidden(
      hidden.includes(outcome) ? hidden.filter((at) => at !== outcome) : [...hidden, outcome],
    );

  return (
    <SectionCard
      title={m.changes_title()}
      description={m.changes_description()}
      action={
        all.length > 0 && (
          <span className="shrink-0 text-xs text-surface-400 select-none">
            {m.changes_values_label({ count: all.length })},{" "}
            {m.changes_objects_label({ count: objects.length })}
          </span>
        )
      }
    >
      {all.length === 0 && (
        <p className="text-sm text-surface-400 select-none">{m.changes_empty()}</p>
      )}
      {all.length > 0 && (
        <div className="flex flex-wrap items-center gap-3" data-ui="ChangesExplorer:toolbar">
          <label className="flex min-w-56 flex-1 items-center gap-2 rounded-md border border-surface-600 bg-surface-950/40 px-2 focus-within:outline-2 focus-within:outline-accent-500">
            <MagnifyingGlassIcon className="h-3.5 w-3.5 shrink-0 text-surface-400" />
            <input
              type="search"
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
              placeholder={m.changes_filter_placeholder()}
              aria-label={m.changes_filter_label()}
              className="h-8 min-w-0 flex-1 bg-transparent font-mono text-xs text-surface-100 outline-none placeholder:font-sans placeholder:text-surface-500"
            />
          </label>
          <span className="flex flex-wrap gap-1.5">
            {OUTCOMES.filter((outcome) => totals[outcome] > 0).map((outcome) => (
              <button
                key={outcome}
                type="button"
                aria-pressed={!hidden.includes(outcome)}
                onClick={() => toggle(outcome)}
                title={m[`changes_outcome.${outcome}.description`]()}
                className={twMerge(
                  "flex h-7 cursor-pointer items-center gap-1.5 rounded-md border px-2 text-xs select-none",
                  hidden.includes(outcome)
                    ? "border-surface-700 text-surface-500"
                    : "border-surface-500 text-surface-100",
                )}
              >
                <OutcomeDot outcome={outcome} />
                {totals[outcome]} {m[`changes_outcome.${outcome}.label`]()}
              </button>
            ))}
          </span>
          <SegmentedControl
            size="xs"
            options={groupings}
            value={grouping}
            onChange={setGrouping}
            aria-label={m.changes_group_label()}
          />
        </div>
      )}
      {all.length > 0 && shown.length === 0 && (
        <p className="text-sm text-surface-400 select-none">{m.changes_none_match()}</p>
      )}
      {shown.length > 0 && grouping === "fixup" && (
        <div className="flex flex-col" data-ui="ChangesExplorer:fixups">
          <Paged items={groupBy(shown, (item) => item.fixup)}>
            {(groups) =>
              groups.map((group) => (
                <FixupGroup
                  key={group.key}
                  fixup={group.key}
                  title={titles.get(group.key)}
                  findings={fixupFindings(group.key)}
                  items={group.items}
                  onField={setFilter}
                />
              ))
            }
          </Paged>
        </div>
      )}
      {shown.length > 0 && grouping === "object" && (
        <ObjectGroups items={shown} fixupLabel={(fixup) => titles.get(fixup) ?? fixup} />
      )}
    </SectionCard>
  );
}
