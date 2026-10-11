import { MagnifyingGlassIcon } from "@phosphor-icons/react";
import type { UseQueryResult } from "@tanstack/react-query";
import { useMemo, useState } from "react";

import { FailureNote, SegmentedControl, Spinner, Switch } from "@/components";
import { m, stageLabel } from "@/i18n";
import type { FileRow, RunView, StageFiles, StageName } from "@/lib/bindings.gen";

import type { Picked } from "../jump";
import { buildTree, filtering, sourcesOf } from "../tree";
import { FileTree, sourceLabel } from "./FileTree";
import { type FileQuery, FileViewer } from "./FileViewer";

/** The stages whose snapshot names files, as the store's `FILE_STAGES` lists them. */
export const FILE_STAGES: readonly StageName[] = ["load", "fix", "plan", "build"];

/** The stages a run ran that hold files, in order. None before its view is read. */
export function heldStages(view: RunView | null | undefined): StageName[] {
  const ran = view?.stages.filter((stage) => stage.state === "ran") ?? [];
  return ran.map((stage) => stage.stage).filter((stage) => FILE_STAGES.includes(stage));
}

/** The source filter's value for every source. */
const ALL_SOURCES = "";

export interface FilesBrowserProps {
  /** The files at the stage, as a query answers them. */
  files: UseQueryResult<StageFiles | null>;
  /** The stages that hold files, in order. A row of them shows when there are two or more. */
  stages: StageName[];
  stage: StageName;
  onStage: (stage: StageName) => void;
  /** The read of a file's bytes, a bin's object of `entry` first. */
  fileQuery: (row: FileRow, entry: string | null) => FileQuery;
  /** Whether the marks are against another run's files, not the stage before. */
  againstRun?: boolean;
  /** Whether the files are marked against anything, which a switch then narrows to. */
  marked?: boolean;
  /** The file picked, for a page that keeps it so a jump to a value's bin can pick one. */
  picked?: Picked | null;
  onPick?: (picked: Picked | null) => void;
}

/**
 * Files at a stage: a tree of their WADs, each file with its mark, and the file picked opened
 * below it. With marks against another run, only the files that differ are shown at first.
 * Files that carry a provenance fact are filtered by it too.
 */
export function FilesBrowser({
  files,
  stages,
  stage,
  onStage,
  fileQuery,
  againstRun = false,
  marked = true,
  picked: kept,
  onPick,
}: FilesBrowserProps) {
  const [text, setText] = useState("");
  const [changedOnly, setChangedOnly] = useState(againstRun);
  const [source, setSource] = useState(ALL_SOURCES);
  const [own, setOwn] = useState<Picked | null>(null);
  const current = kept === undefined ? own : kept;
  const setPicked = onPick ?? setOwn;
  const picked = current?.row ?? null;
  const sources = useMemo(() => sourcesOf(files.data?.wads ?? []), [files.data]);
  const filter = useMemo(
    () => ({ text, changedOnly, source: source === ALL_SOURCES ? null : source }),
    [text, changedOnly, source],
  );
  const roots = useMemo(() => buildTree(files.data?.wads ?? [], filter), [files.data, filter]);
  const options = stages.map((value) => ({ value, label: stageLabel(value) }));
  const sourceOptions = [
    { value: ALL_SOURCES, label: m.runs_files_source_all() },
    ...sources.map((value) => ({ value, label: sourceLabel(value) })),
  ];
  const choose = (next: StageName) => {
    setPicked(null);
    onStage(next);
  };

  return (
    <div className="flex flex-col gap-4" data-ui="FilesTab">
      <div className="flex flex-wrap items-center gap-3">
        {stages.length > 1 && (
          <SegmentedControl
            options={options}
            value={stage}
            onChange={choose}
            aria-label={m.runs_files_stage_label()}
          />
        )}
        <label className="flex min-w-48 flex-1 items-center gap-2 rounded-md border border-surface-600 bg-surface-950/40 px-2 focus-within:outline-2 focus-within:outline-accent-500">
          <MagnifyingGlassIcon className="h-3.5 w-3.5 shrink-0 text-surface-400" />
          <input
            type="search"
            value={text}
            onChange={(event) => setText(event.target.value)}
            placeholder={m.runs_files_filter_placeholder()}
            aria-label={m.runs_files_filter_label()}
            className="h-8 min-w-0 flex-1 bg-transparent font-mono text-xs text-surface-100 outline-none placeholder:font-sans placeholder:text-surface-500"
          />
        </label>
        {sources.length > 0 && (
          <SegmentedControl
            options={sourceOptions}
            value={source}
            onChange={setSource}
            aria-label={m.runs_files_source_label()}
          />
        )}
        {marked && (
          <label className="flex items-center gap-2 text-xs text-surface-300 select-none">
            <Switch
              checked={changedOnly}
              onCheckedChange={setChangedOnly}
              aria-label={m.runs_files_changed_only()}
            />
            {m.runs_files_changed_only()}
          </label>
        )}
      </div>
      {files.isPending && <Spinner />}
      {files.isError && <FailureNote failure={files.error} />}
      {files.data && roots.length === 0 && (
        <p className="text-sm text-surface-400 select-none">
          {filtering(filter) ? m.runs_files_none_match() : m.runs_files_empty()}
        </p>
      )}
      {roots.length > 0 && (
        <div className="max-h-[50vh] overflow-y-auto rounded-lg border border-surface-700/50 bg-surface-900/95 p-1">
          <FileTree
            // A stage's tree opens as it does at first.
            key={stage}
            roots={roots}
            allOpen={filtering(filter)}
            selected={picked}
            onPick={(row) => setPicked({ row, entry: null })}
            againstRun={againstRun}
          />
        </div>
      )}
      {current ? (
        <FileViewer
          row={current.row}
          query={fileQuery(current.row, current.entry)}
          entry={current.entry}
        />
      ) : (
        roots.length > 0 && (
          <p className="text-sm text-surface-400 select-none">{m.runs_file_pick_hint()}</p>
        )
      )}
    </div>
  );
}
