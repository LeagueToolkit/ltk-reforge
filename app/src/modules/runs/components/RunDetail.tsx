import { FolderOpenIcon, PushPinIcon, TrashIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { Button, FailureNote, Spinner, TabPanel, Tabs } from "@/components";
import { m } from "@/i18n";
import type { RunRow, RunView, StageName } from "@/lib/bindings.gen";
import { ConfirmRemove } from "@/modules/inputs";
import { endedResults, RunResults, StageStrip } from "@/modules/repair";
import { twMerge } from "@/utils";

import { useKeepRun, useRemoveRun, useRevealRun, useRunReport, useRunView } from "../api/runs";
import { FILE_STAGES, FilesTab } from "./FilesTab";
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

/** The stages a run ran that hold files, in order. */
function heldStages(view: RunView): StageName[] {
  const ran = view.stages.filter((stage) => stage.state === "ran").map((stage) => stage.stage);
  return ran.filter((stage) => FILE_STAGES.includes(stage));
}

/** The report the package embeds, as the store keeps it. */
function ReportTab({ id }: { id: number }) {
  const report = useRunReport(id);
  return (
    <div className="flex flex-col gap-2" data-ui="ReportTab">
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
}

/** A kept run: its mod and package, what can be done with it, its result, files and report. */
export function RunDetail({ row, onRemoved }: RunDetailProps) {
  const view = useRunView(row.id);
  const keep = useKeepRun();
  const remove = useRemoveRun();
  const reveal = useRevealRun();
  const [asking, setAsking] = useState(false);
  const [tab, setTab] = useState<Tab>("result");
  const [filesStage, setFilesStage] = useState<StageName | null>(null);
  const name = runName(row);
  const held = view.data ? heldStages(view.data) : [];
  const tabs = [
    { value: "result" as const, label: m.runs_tab_result() },
    ...(held.length > 0 ? [{ value: "files" as const, label: m.runs_tab_files() }] : []),
    { value: "report" as const, label: m.runs_tab_report() },
  ];
  const openFiles = (stage: StageName) => {
    setFilesStage(stage);
    setTab("files");
  };
  const failure = keep.error ?? remove.error ?? reveal.error;

  return (
    <div className="flex flex-col gap-6" data-ui="RunDetail">
      <div className="flex items-start gap-4">
        <Thumbnail src={view.data?.package?.thumbnail} name={name} />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <p className="text-base font-semibold text-surface-50">{name}</p>
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
        <Button
          size="sm"
          variant="ghost"
          className="ml-auto"
          left={<TrashIcon weight="bold" />}
          loading={remove.isPending}
          onClick={() => setAsking(true)}
        >
          {m.runs_remove_action()}
        </Button>
        <ConfirmRemove
          open={asking}
          onClose={() => setAsking(false)}
          title={m.runs_remove_title()}
          description={m.runs_remove_description()}
          onConfirm={() => remove.mutate(row.id, { onSuccess: onRemoved })}
        />
      </div>
      {failure && <FailureNote failure={failure} />}
      {view.isPending && <Spinner />}
      {view.isError && <FailureNote failure={view.error} />}
      {view.data && (
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
          <TabPanel value="report">
            <ReportTab id={row.id} />
          </TabPanel>
        </Tabs>
      )}
    </div>
  );
}
