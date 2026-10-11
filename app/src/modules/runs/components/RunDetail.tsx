import {
  ArrowClockwiseIcon,
  ArrowsLeftRightIcon,
  ExportIcon,
  FolderOpenIcon,
  PushPinIcon,
} from "@phosphor-icons/react";
import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";

import { Button, FailureNote, Spinner, TabPanel, Tabs, Tooltip } from "@/components";
import { m } from "@/i18n";
import type { RunRow, RunView, StageName } from "@/lib/bindings.gen";
import { pickSavePath } from "@/modules/inputs";
import { endedResults, RunResults, StageStrip, useRepairStore } from "@/modules/repair";
import { twMerge } from "@/utils";

import { PROJECT_EXTENSION } from "../api/drop";
import {
  useExportProject,
  useKeepRun,
  useRevealRun,
  useRunAgain,
  useRunReport,
  useRunView,
  useRuns,
} from "../api/runs";
import { ComparePicker } from "./ComparePicker";
import { CompareView } from "./CompareView";
import { FilesTab, heldStages } from "./FilesTab";
import { RemoveMenu } from "./RemoveMenu";
import { runName } from "./RunList";

/** The tabs of a run. */
type Tab = "result" | "files" | "report";

/** The package's thumbnail, else the first letter of the run's name. */
function Thumbnail({ src, name }: { src: string | null | undefined; name: string }) {
  const box = "h-14 w-14 shrink-0 rounded-md border border-surface-700";
  if (src) {
    return <img src={src} alt={m.runs_thumbnail_alt()} className={twMerge(box, "object-cover")} />;
  }
  return (
    <span
      aria-hidden
      className={twMerge(
        box,
        "flex items-center justify-center bg-surface-800 font-display text-xl font-semibold text-surface-300 select-none",
      )}
    >
      {name.slice(0, 1).toUpperCase()}
    </span>
  );
}

/**
 * The run's result as the run page shows one that ended: its stages and what it found and did.
 * A stage picked that holds files opens them.
 */
function ResultTab({ view, onFiles }: { view: RunView; onFiles: (stage: StageName) => void }) {
  const last = view.stages.filter((stage) => stage.state !== "not-run").at(-1)?.stage ?? "read";
  const [selected, setSelected] = useState<StageName>(last);
  const pick = (stage: StageName) => {
    setSelected(stage);
    if (heldStages(view).includes(stage)) {
      onFiles(stage);
    }
  };
  return (
    <>
      <StageStrip
        stages={view.stages}
        findings={view.findings}
        selected={selected}
        onSelect={pick}
      />
      <RunResults {...endedResults(view)} />
    </>
  );
}

/** The line a tab of a run opened from a package shows: it has no snapshots. */
function NoSnapshots() {
  return (
    <p className="text-sm text-surface-400 select-none" data-ui="RunDetail:no-snapshots">
      {m.runs_no_snapshots()}
    </p>
  );
}

/** Where a run came from, when not from a run of the app's, with the path it was opened from. */
function OriginBadge({ row }: { row: RunRow }) {
  if (row.origin === "run") {
    return null;
  }
  const label = row.origin === "project" ? m.runs_origin_project() : m.runs_origin_package();
  const badge = (
    <span
      className="shrink-0 rounded-full border border-surface-600 px-2 py-0.5 text-[11px] font-normal text-surface-300 select-none"
      data-ui="RunDetail:origin"
    >
      {label}
    </span>
  );
  return row.opened_from ? (
    <Tooltip content={m.runs_origin_from({ path: row.opened_from })}>{badge}</Tooltip>
  ) : (
    badge
  );
}

/** A file name without its last extension. */
function stemOf(fileName: string): string {
  return fileName.replace(/\.[^.]+$/, "");
}

/** The report the package embeds, as the store keeps it. */
function ReportTab({ id, opened }: { id: number; opened: boolean }) {
  const report = useRunReport(id);
  return (
    <div className="flex flex-col gap-2" data-ui="ReportTab">
      {opened && <NoSnapshots />}
      <p className="text-xs text-surface-400 select-none">{m.runs_report_note()}</p>
      {report.isPending && <Spinner />}
      {report.isError && <FailureNote failure={report.error} />}
      {report.data && (
        <pre className="max-h-[70vh] overflow-auto rounded-lg border border-surface-700/50 bg-surface-900/95 p-4 font-mono text-xs text-surface-200">
          {report.data}
        </pre>
      )}
    </div>
  );
}

interface RunDetailProps {
  row: RunRow;
  /** Called once the run is removed. */
  onRemoved: () => void;
  /** The time the ages are counted to. */
  now: Date;
}

/**
 * A kept run: its mod and package, what can be done with it, and its result, files and report,
 * or its comparison with another run.
 */
export function RunDetail({ row, onRemoved, now }: RunDetailProps) {
  const view = useRunView(row.id);
  const keep = useKeepRun();
  const reveal = useRevealRun();
  const again = useRunAgain(row.id);
  const runs = useRuns();
  const modRuns = runs.data?.filter((other) => other.sha256 === row.sha256).length ?? 1;
  const takeRun = useRepairStore((state) => state.takeRun);
  const navigate = useNavigate();
  const [tab, setTab] = useState<Tab>("result");
  const [filesStage, setFilesStage] = useState<StageName | null>(null);
  const [picking, setPicking] = useState(false);
  const [against, setAgainst] = useState<number | null>(null);
  const exportProject = useExportProject();
  const name = runName(row);
  // A package opened has no snapshots, no settings and no bytes (#ADR-0096).
  const opened = row.origin === "package";
  const held = view.data ? heldStages(view.data) : [];
  const tabs = [
    { value: "result" as const, label: m.runs_tab_result() },
    ...(held.length > 0 || opened ? [{ value: "files" as const, label: m.runs_tab_files() }] : []),
    { value: "report" as const, label: m.runs_tab_report() },
  ];
  const openFiles = (stage: StageName) => {
    setFilesStage(stage);
    setTab("files");
  };
  const failure = keep.error ?? reveal.error ?? again.error ?? exportProject.error;
  const canRunAgain = !opened && again.data?.archive_present;
  const againButton = (
    <Button
      size="sm"
      variant="outline"
      left={<ArrowClockwiseIcon weight="bold" />}
      disabled={!canRunAgain}
      loading={again.isPending}
      title={canRunAgain ? m.runs_again_description() : undefined}
      onClick={() => {
        if (again.data) {
          takeRun(again.data);
          void navigate({ to: "/" });
        }
      }}
    >
      {m.runs_again_action()}
    </Button>
  );
  const exportTo = async () => {
    const path = await pickSavePath(
      [PROJECT_EXTENSION],
      m.runs_export_filter(),
      `${stemOf(row.file_name)}.${PROJECT_EXTENSION}`,
    );
    if (path) {
      exportProject.mutate({ id: row.id, path });
    }
  };
  const exportButton = (
    <Button
      size="sm"
      variant="outline"
      left={<ExportIcon weight="bold" />}
      disabled={opened}
      loading={exportProject.isPending}
      title={opened ? undefined : m.runs_export_description()}
      onClick={() => void exportTo()}
    >
      {m.runs_export_action()}
    </Button>
  );

  return (
    <div className="flex flex-col gap-6" data-ui="RunDetail">
      <div className="flex items-start gap-4">
        <Thumbnail src={view.data?.package?.thumbnail} name={name} />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <p className="flex items-center gap-2 text-base font-semibold text-surface-50">
            <span className="min-w-0 truncate">{name}</span>
            <OriginBadge row={row} />
          </p>
          {(row.author || row.version) && (
            <p className="text-xs text-surface-300">
              {[
                row.author && m.runs_by_label({ author: row.author }),
                row.version && m.runs_version_label({ version: row.version }),
              ]
                .filter(Boolean)
                .join(", ")}
            </p>
          )}
          <p className="font-mono text-xs break-all text-surface-400">{row.file_name}</p>
        </div>
      </div>
      <div className="flex flex-wrap items-center gap-2" data-ui="RunDetail:actions">
        <Button
          size="sm"
          variant={row.kept ? "light" : "outline"}
          aria-pressed={row.kept}
          title={m.runs_keep_description()}
          left={<PushPinIcon weight={row.kept ? "fill" : "bold"} />}
          loading={keep.isPending}
          onClick={() => keep.mutate({ id: row.id, kept: !row.kept })}
        >
          {m.runs_keep_action()}
        </Button>
        <Button
          size="sm"
          variant="outline"
          left={<FolderOpenIcon weight="bold" />}
          disabled={!row.package}
          loading={reveal.isPending}
          onClick={() => reveal.mutate(row.id)}
        >
          {m.runs_reveal_action()}
        </Button>
        {opened || (again.data && !again.data.archive_present) ? (
          // A disabled button hears no pointer, so the note is on what holds it.
          <Tooltip content={opened ? m.runs_again_package() : m.runs_again_missing()}>
            <span>{againButton}</span>
          </Tooltip>
        ) : (
          againButton
        )}
        {opened ? (
          <Tooltip content={m.runs_export_package()}>
            <span>{exportButton}</span>
          </Tooltip>
        ) : (
          exportButton
        )}
        <Button
          size="sm"
          variant={against == null ? "outline" : "light"}
          left={<ArrowsLeftRightIcon weight="bold" />}
          disabled={(runs.data?.length ?? 0) < 2}
          title={m.compare_action_description()}
          onClick={() => setPicking(true)}
        >
          {m.compare_action()}
        </Button>
        <RemoveMenu row={row} modRuns={modRuns} onRemoved={onRemoved} />
      </div>
      <ComparePicker
        open={picking}
        onClose={() => setPicking(false)}
        row={row}
        runs={runs.data ?? []}
        onPick={setAgainst}
        now={now}
      />
      {failure && <FailureNote failure={failure} />}
      {against != null && (
        <CompareView
          // A comparison opens at its top, its files at their last stage.
          key={against}
          a={row.id}
          b={against}
          onBack={() => setAgainst(null)}
          now={now}
        />
      )}
      {against == null && view.isPending && <Spinner />}
      {against == null && view.isError && <FailureNote failure={view.error} />}
      {against == null && view.data && (
        <Tabs tabs={tabs} value={tab} onChange={setTab} aria-label={m.runs_tabs_label()}>
          <TabPanel value="result">
            <ResultTab view={view.data} onFiles={openFiles} />
          </TabPanel>
          {held.length > 0 && (
            <TabPanel value="files">
              <FilesTab
                id={row.id}
                stages={held}
                stage={filesStage ?? held[held.length - 1]}
                onStage={setFilesStage}
              />
            </TabPanel>
          )}
          {held.length === 0 && opened && (
            <TabPanel value="files">
              <NoSnapshots />
            </TabPanel>
          )}
          <TabPanel value="report">
            <ReportTab id={row.id} opened={opened} />
          </TabPanel>
        </Tabs>
      )}
    </div>
  );
}
