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
import { twMerge } from "@/utils";

import { useCompare, useCompareFiles, useRunView } from "../api/runs";
import { differingFirst, isDiffKey, SWITCH_KEYS } from "../compare";
import { FilesBrowser, heldStages } from "./FilesTab";
import { KindBadge, RunWhen } from "./RunList";

const tableClass = "w-full text-left text-xs";
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
  return value;
}

/** The name of a setting or an input; the key itself for one the window does not know. */
function diffLabel(key: string): string {
  return isDiffKey(key) ? m[`compare_key_${key}`]() : key;
}

function SettingsSection({ rows }: { rows: Diff[] }) {
  return (
    <SectionCard title={m.compare_settings_title()} description={m.compare_settings_description()}>
      {rows.length === 0 ? (
        <Empty>{m.compare_settings_empty()}</Empty>
      ) : (
        <table className={tableClass} data-ui="CompareView:settings">
          <thead>
            <tr>
              <th className={headClass}>{m.compare_column_what()}</th>
              <th className={headClass}>{m.compare_side_a()}</th>
              <th className={headClass}>{m.compare_side_b()}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-surface-700/50">
            {rows.map((row) => (
              <tr key={row.key}>
                <td className={twMerge(cellClass, "text-surface-200")}>{diffLabel(row.key)}</td>
                <td className={twMerge(cellClass, "font-mono break-all text-surface-100")}>
                  {diffValue(row.key, row.a)}
                </td>
                <td className={twMerge(cellClass, "font-mono break-all text-surface-100")}>
                  {diffValue(row.key, row.b)}
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

/** A unit's target in one run, and why. */
function TargetCell({ chosen, reason }: { chosen: number | null; reason: string }) {
  if (reason === "") {
    return <span className="text-surface-500">{m.compare_value_absent()}</span>;
  }
  return (
    <span className="flex flex-col">
      <span className="text-surface-100">
        {chosen == null ? m.decide_left_to_game_label() : m.decide_skin_label({ slot: chosen })}
      </span>
      <span className="font-mono text-surface-400">{reason}</span>
    </span>
  );
}

/** A row's mark when it differs between the runs. */
function DiffersDot({ differs }: { differs: boolean }) {
  if (!differs) {
    return null;
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
              <th className={headClass}>{m.compare_column_unit()}</th>
              <th className={headClass}>{m.compare_side_a()}</th>
              <th className={headClass}>{m.compare_side_b()}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-surface-700/50">
            {differingFirst(targets).map((target) => (
              <tr key={target.unit}>
                <td className={twMerge(cellClass, "text-surface-200")}>
                  <DiffersDot differs={target.differs} />
                  {target.unit}
                </td>
                <td className={cellClass}>
                  <TargetCell chosen={target.a} reason={target.a_reason} />
                </td>
                <td className={cellClass}>
                  <TargetCell chosen={target.b} reason={target.b_reason} />
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
              <th className={headClass}>{m.compare_column_fixup()}</th>
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
