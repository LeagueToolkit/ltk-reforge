import { ArrowLeftIcon, MinusIcon, PlusIcon } from "@phosphor-icons/react";
import { type ReactNode, useState } from "react";

import { Button, FailureNote, SectionCard, Spinner } from "@/components";
import { m } from "@/i18n";
import type {
  ChangeDiff,
  CompareView as Compared,
  Counts,
  Diff,
  Finding,
  PackageView,
  RunHead,
  RunView,
  StageName,
  TargetDiff,
} from "@/lib/bindings.gen";
import { FindingChanges, isFixupFinding } from "@/modules/changes";
import { FindingList } from "@/modules/findings";
import { useFixupCatalog } from "@/modules/fixups";
import { reasonTitle } from "@/modules/repair";
import { twMerge } from "@/utils";

import { useCompare, useCompareFiles, useRunView } from "../api/runs";
import { differingFirst, isDiffKey, SWITCH_KEYS } from "../compare";
import { FilesBrowser, heldStages } from "./FilesTab";
import { KindBadge, RunWhen } from "./RunList";

// Fixed columns: a long value wraps in its own column and leaves the other run's its width.
const tableClass = "w-full table-fixed text-left text-xs";
const keyColumn = "w-1/4";
const headClass = "pb-2 font-medium text-surface-400 select-none";
const cellClass = "py-1.5 pr-3 align-top";

/** A section's line when it holds nothing. */
function Empty({ children }: { children: ReactNode }) {
  return <p className="text-sm text-surface-400 select-none">{children}</p>;
}

/** One run of the comparison, as its column of the header shows it. */
function HeadColumn({ head, side, now }: { head: RunHead; side: string; now: Date }) {
  return (
    <div className="flex min-w-0 flex-col gap-1" data-ui="CompareView:head">
      <span className="text-[11px] font-semibold tracking-wide text-surface-400 uppercase select-none">
        {side}
      </span>
      <span className="truncate text-sm font-semibold text-surface-50">
        {head.name ?? head.file_name}
      </span>
      <span className="flex flex-wrap items-center gap-2 text-xs text-surface-400">
        <KindBadge kind={head.kind} />
        {head.version && <span>{m.runs_version_label({ version: head.version })}</span>}
        <RunWhen madeAt={head.made_at} now={now} />
      </span>
      <span className="font-mono text-xs break-all text-surface-400">
        {head.package ?? m.compare_no_package()}
      </span>
    </div>
  );
}

/** A value of a setting or an input for a person: a switch as on or off, none as such. */
function diffValue(key: string, value: string): string {
  if (value === "") {
    return m.compare_value_none();
  }
  if (key === "kind" && (value === "repair" || value === "port")) {
    return m[`runs_kind.${value}.label`]();
  }
  if (isDiffKey(key) && SWITCH_KEYS.includes(key)) {
    return value === "true" ? m.compare_value_on() : m.compare_value_off();
  }
  if (key === "names") {
    return tablesLabel(value) ?? value;
  }
  return value;
}

/** A hash table as the load snapshot names it. */
interface TableStamp {
  table: string;
  version: string;
  sha256?: string;
}

function isTableStamp(value: unknown): value is TableStamp {
  const stamp = value as TableStamp;
  return typeof stamp?.table === "string" && typeof stamp.version === "string";
}

/**
 * The hash tables a run read, one a line: its name, version and the start of its SHA-256,
 * which tells two copies of one version apart. None when the value is not the tables' JSON.
 */
function tablesLabel(value: string): string | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(value);
  } catch {
    return null;
  }
  const lists = Object.values(parsed as Record<string, unknown>).filter(Array.isArray);
  const stamps = lists.flat().filter(isTableStamp);
  if (stamps.length === 0) {
    return null;
  }
  return stamps
    .map((stamp) =>
      [stamp.table, stamp.version, stamp.sha256?.slice(0, 8)].filter(Boolean).join(" "),
    )
    .join("\n");
}

/** The fixups a value lists, by title, each with its id. */
function FixupsValue({ value, titles }: { value: string; titles: Map<string, string> }) {
  return (
    <span className="flex flex-col gap-1">
      {value.split(", ").map((id) => (
        <span key={id} className="flex flex-col">
          <span className="font-sans text-surface-100">{titles.get(id) ?? id}</span>
          <span className="text-surface-400">{id}</span>
        </span>
      ))}
    </span>
  );
}

/** One run's value of a setting or an input. */
function DiffValue({
  name,
  value,
  titles,
}: {
  name: string;
  value: string;
  titles: Map<string, string>;
}) {
  if (value !== "" && (name === "fixups_off" || name === "fixups_on")) {
    return <FixupsValue value={value} titles={titles} />;
  }
  return <>{diffValue(name, value)}</>;
}

/** The name of a setting or an input; the key itself for one the window does not know. */
function diffLabel(key: string): string {
  return isDiffKey(key) ? m[`compare_key_${key}`]() : key;
}

function SettingsSection({ rows }: { rows: Diff[] }) {
  const catalog = useFixupCatalog();
  const titles = new Map(catalog.data?.map((entry) => [entry.id, entry.title]));
  const valueClass = twMerge(
    cellClass,
    "font-mono wrap-anywhere whitespace-pre-line text-surface-100",
  );
  return (
    <SectionCard title={m.compare_settings_title()} description={m.compare_settings_description()}>
      {rows.length === 0 ? (
        <Empty>{m.compare_settings_empty()}</Empty>
      ) : (
        <table className={tableClass} data-ui="CompareView:settings">
          <thead>
            <tr>
              <th className={twMerge(headClass, keyColumn)}>{m.compare_column_what()}</th>
              <th className={headClass}>{m.compare_side_a()}</th>
              <th className={headClass}>{m.compare_side_b()}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-surface-700/50">
            {rows.map((row) => (
              <tr key={row.key}>
                <td className={twMerge(cellClass, "text-surface-200")}>{diffLabel(row.key)}</td>
                <td className={valueClass}>
                  <DiffValue name={row.key} value={row.a} titles={titles} />
                </td>
                <td className={valueClass}>
                  <DiffValue name={row.key} value={row.b} titles={titles} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </SectionCard>
  );
}

/** What a run's finding opens to: a fixup's finding, the changes it counts. */
function detailIn(view: RunView | null | undefined) {
  const fixed = view?.stages.some((stage) => stage.stage === "fix" && stage.state === "ran");
  return function detailOf(finding: Finding): ReactNode {
    if (!view || !fixed || !isFixupFinding(finding)) {
      return null;
    }
    return <FindingChanges finding={finding} changes={view.changes} objects={view.objects} />;
  };
}

interface FindingsColumnProps {
  title: string;
  icon: ReactNode;
  tone: string;
  findings: Finding[];
  view: RunView | null | undefined;
  empty: string;
}

function FindingsColumn({ title, icon, tone, findings, view, empty }: FindingsColumnProps) {
  return (
    <div className="flex min-w-0 flex-col gap-2">
      <span
        className={twMerge(
          "flex items-center gap-1.5 self-start rounded-full px-2 py-0.5 text-xs font-medium select-none",
          tone,
        )}
      >
        {icon}
        {title}
      </span>
      <FindingList
        findings={findings}
        empty={empty}
        detailOf={detailIn(view)}
        names={view?.names}
      />
    </div>
  );
}

function FindingsSection({
  compared,
  viewA,
  viewB,
}: {
  compared: Compared;
  viewA: RunView | null | undefined;
  viewB: RunView | null | undefined;
}) {
  const { added, gone, same } = compared.findings;
  return (
    <SectionCard title={m.compare_findings_title()} description={m.compare_same({ count: same })}>
      {added.length === 0 && gone.length === 0 ? (
        <Empty>{m.compare_findings_empty()}</Empty>
      ) : (
        <div className="grid grid-cols-2 gap-6" data-ui="CompareView:findings">
          <FindingsColumn
            title={m.compare_findings_gone({ count: gone.length })}
            icon={<MinusIcon weight="bold" className="h-3 w-3" />}
            tone="bg-danger/10 text-danger-text"
            findings={gone}
            view={viewA}
            empty={m.compare_findings_none_gone()}
          />
          <FindingsColumn
            title={m.compare_findings_added({ count: added.length })}
            icon={<PlusIcon weight="bold" className="h-3 w-3" />}
            tone="bg-success/10 text-success-text"
            findings={added}
            view={viewB}
            empty={m.compare_findings_none_added()}
          />
        </div>
      )}
    </SectionCard>
  );
}

/** A unit's target in one run, and why, as the run page says it. */
function TargetCell({
  chosen,
  reason,
  character,
}: {
  chosen: number | null;
  reason: string;
  character: string;
}) {
  if (reason === "") {
    return <span className="text-surface-500">{m.compare_value_absent()}</span>;
  }
  const why = reasonTitle(reason, character, chosen);
  return (
    <span className="flex flex-col">
      <span className="text-surface-100">
        {chosen == null ? m.decide_left_to_game_label() : m.decide_skin_label({ slot: chosen })}
      </span>
      {why ? (
        <span className="text-surface-400">{why}</span>
      ) : (
        <span className="font-mono text-surface-400">{reason}</span>
      )}
    </span>
  );
}

/** A row's mark when it differs between the runs; its room kept on a row that does not. */
function DiffersDot({ differs }: { differs: boolean }) {
  if (!differs) {
    return <span aria-hidden className="mr-2 inline-block h-2 w-2 shrink-0" />;
  }
  return (
    <span
      role="img"
      aria-label={m.compare_differs_label()}
      title={m.compare_differs_label()}
      className="mr-2 inline-block h-2 w-2 shrink-0 rounded-full bg-warning"
    />
  );
}

function TargetsSection({ targets }: { targets: TargetDiff[] }) {
  return (
    <SectionCard title={m.compare_targets_title()}>
      {targets.length === 0 ? (
        <Empty>{m.compare_targets_empty()}</Empty>
      ) : (
        <table className={tableClass} data-ui="CompareView:targets">
          <thead>
            <tr>
              <th className={twMerge(headClass, keyColumn)}>{m.compare_column_unit()}</th>
              <th className={headClass}>{m.compare_side_a()}</th>
              <th className={headClass}>{m.compare_side_b()}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-surface-700/50">
            {differingFirst(targets).map((target) => (
              <tr key={target.unit}>
                <td className={twMerge(cellClass, "text-surface-200")}>
                  <span className="flex items-start">
                    <DiffersDot differs={target.differs} />
                    <span>{target.unit}</span>
                  </span>
                </td>
                <td className={cellClass}>
                  <TargetCell
                    chosen={target.a}
                    reason={target.a_reason}
                    character={target.character_name}
                  />
                </td>
                <td className={cellClass}>
                  <TargetCell
                    chosen={target.b}
                    reason={target.b_reason}
                    character={target.character_name}
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </SectionCard>
  );
}

const OUTCOMES = ["changed", "removed", "lost", "unconvertible"] as const;

/** A fixup's counts in one run: each outcome it has, else none. */
function countsLabel(counts: Counts): string {
  const made = OUTCOMES.filter((outcome) => counts[outcome] > 0);
  if (made.length === 0) {
    return m.compare_value_none();
  }
  return made
    .map((outcome) =>
      m.compare_count({
        count: counts[outcome],
        outcome: m[`changes_outcome.${outcome}.label`](),
      }),
    )
    .join(", ");
}

function FixupsSection({ changes }: { changes: ChangeDiff[] }) {
  const catalog = useFixupCatalog();
  const titles = new Map(catalog.data?.map((entry) => [entry.id, entry.title]));
  return (
    <SectionCard title={m.compare_fixups_title()}>
      {changes.length === 0 ? (
        <Empty>{m.compare_fixups_empty()}</Empty>
      ) : (
        <table className={tableClass} data-ui="CompareView:fixups">
          <thead>
            <tr>
              <th className={twMerge(headClass, "w-1/2")}>{m.compare_column_fixup()}</th>
              <th className={headClass}>{m.compare_side_a()}</th>
              <th className={headClass}>{m.compare_side_b()}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-surface-700/50">
            {differingFirst(changes).map((change) => (
              <tr key={change.fixup}>
                <td className={cellClass}>
                  <span className="flex items-start">
                    <DiffersDot differs={change.differs} />
                    <span className="flex flex-col">
                      <span className="text-surface-100">
                        {titles.get(change.fixup) ?? change.fixup}
                      </span>
                      <span className="font-mono text-surface-400">{change.fixup}</span>
                    </span>
                  </span>
                </td>
                <td className={twMerge(cellClass, "text-surface-200")}>{countsLabel(change.a)}</td>
                <td className={twMerge(cellClass, "text-surface-200")}>{countsLabel(change.b)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </SectionCard>
  );
}

/** One run's package: its thumbnail, name, version and what it holds. */
function PackageColumn({ side, view }: { side: string; view: PackageView | null }) {
  return (
    <div className="flex min-w-0 flex-col gap-2">
      <span className="text-[11px] font-semibold tracking-wide text-surface-400 uppercase select-none">
        {side}
      </span>
      {view ? (
        <div className="flex items-start gap-3">
          {view.thumbnail && (
            <img
              src={view.thumbnail}
              alt={m.runs_thumbnail_alt()}
              className="h-14 w-14 shrink-0 rounded-md border border-surface-700 object-cover"
            />
          )}
          <p className="text-sm text-surface-200">
            {m.advanced_package_label({
              name: view.name,
              version: view.version,
              chunks: view.chunks,
              patches: view.patches,
            })}
          </p>
        </div>
      ) : (
        <Empty>{m.compare_no_package()}</Empty>
      )}
    </div>
  );
}

function PackageSection({ compared }: { compared: Compared }) {
  return (
    <SectionCard title={m.compare_package_title()}>
      <div className="grid grid-cols-2 gap-6" data-ui="CompareView:package">
        <PackageColumn side={m.compare_side_a()} view={compared.package.a} />
        <PackageColumn side={m.compare_side_b()} view={compared.package.b} />
      </div>
    </SectionCard>
  );
}

function FilesSection({ a, b, stages }: { a: number; b: number; stages: StageName[] }) {
  const [stage, setStage] = useState<StageName>(stages[stages.length - 1]);
  const files = useCompareFiles(a, b, stage);
  return (
    <SectionCard title={m.compare_files_title()} description={m.compare_files_description()}>
      <FilesBrowser
        files={files}
        stages={stages}
        stage={stage}
        onStage={setStage}
        // A file removed is the first run's; every other is the second's.
        runOf={(row) => (row.mark === "removed" ? a : b)}
        againstRun
      />
    </SectionCard>
  );
}

interface CompareViewProps {
  /** The run that was open, on the left. */
  a: number;
  /** The run picked, on the right. */
  b: number;
  onBack: () => void;
  /** The time the ages are counted to. */
  now: Date;
}

/**
 * Two kept runs compared (#ADR-0091): what differs in their settings and inputs, their
 * findings, a port's targets, the fixups' counts, their packages and their files at a stage.
 * A diff of what the store answers, which adds no rule about a mod (#ADR-0046).
 */
export function CompareView({ a, b, onBack, now }: CompareViewProps) {
  const compare = useCompare(a, b);
  const viewA = useRunView(a);
  const viewB = useRunView(b);
  const compared = compare.data;
  const held = heldStages(viewB.data);
  const stages = heldStages(viewA.data).filter((stage) => held.includes(stage));

  return (
    <div className="flex flex-col gap-6" data-ui="CompareView">
      <div>
        <Button size="sm" variant="ghost" left={<ArrowLeftIcon weight="bold" />} onClick={onBack}>
          {m.compare_back_action()}
        </Button>
      </div>
      {compare.isPending && <Spinner />}
      {compare.isError && <FailureNote failure={compare.error} />}
      {compare.isSuccess && !compared && <Empty>{m.compare_gone()}</Empty>}
      {compared && (
        <>
          <div className="grid grid-cols-2 gap-6 rounded-xl border border-surface-700/50 bg-surface-900/95 p-4">
            <HeadColumn head={compared.a} side={m.compare_side_a()} now={now} />
            <HeadColumn head={compared.b} side={m.compare_side_b()} now={now} />
            {!compared.same_mod && (
              <p className="col-span-2 text-xs text-warning-text" data-ui="CompareView:other-mod">
                {m.compare_different_mods()}
              </p>
            )}
          </div>
          <SettingsSection rows={[...compared.settings, ...compared.inputs]} />
          <FindingsSection compared={compared} viewA={viewA.data} viewB={viewB.data} />
          {compared.a.kind === "port" && compared.b.kind === "port" && (
            <TargetsSection targets={compared.targets} />
          )}
          <FixupsSection changes={compared.changes} />
          <PackageSection compared={compared} />
          {stages.length > 0 ? (
            <FilesSection a={a} b={b} stages={stages} />
          ) : (
            viewA.data &&
            viewB.data && (
              <SectionCard title={m.compare_files_title()}>
                <Empty>{m.compare_files_empty()}</Empty>
              </SectionCard>
            )
          )}
        </>
      )}
    </div>
  );
}
