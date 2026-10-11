import { MagnifyingGlassIcon } from "@phosphor-icons/react";
import { useMemo, useState } from "react";

import { FailureNote, SegmentedControl, Spinner, Switch } from "@/components";
import { m } from "@/i18n";
import type { FileRow, StageName } from "@/lib/bindings.gen";
import { stageLabel } from "@/modules/repair";

import { useRunFiles } from "../api/runs";
import { buildTree, filtering } from "../tree";
import { FileTree } from "./FileTree";
import { FileViewer } from "./FileViewer";

/** The stages whose snapshot names files, as the store's `FILE_STAGES` lists them. */
export const FILE_STAGES: readonly StageName[] = ["load", "fix", "plan", "build"];

interface FilesTabProps {
  id: number;
  /** The stages the run holds files for, in order. */
  stages: StageName[];
  stage: StageName;
  onStage: (stage: StageName) => void;
}

/**
 * The files of a kept run at each stage it holds files for: a tree of its WADs, each file
 * marked against the stage before, and the file picked opened below it.
 */
export function FilesTab({ id, stages, stage, onStage }: FilesTabProps) {
  const [text, setText] = useState("");
  const [changedOnly, setChangedOnly] = useState(false);
  const [picked, setPicked] = useState<FileRow | null>(null);
  const files = useRunFiles(id, stage);
  const filter = useMemo(() => ({ text, changedOnly }), [text, changedOnly]);
  const roots = useMemo(() => buildTree(files.data?.wads ?? [], filter), [files.data, filter]);
  const options = stages.map((value) => ({ value, label: stageLabel(value) }));
  const choose = (next: StageName) => {
    setPicked(null);
    onStage(next);
  };

  return (
    <div className="flex flex-col gap-4" data-ui="FilesTab">
      <div className="flex flex-wrap items-center gap-3">
        <SegmentedControl
          options={options}
          value={stage}
          onChange={choose}
          aria-label={m.runs_files_stage_label()}
        />
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
        <label className="flex items-center gap-2 text-xs text-surface-300 select-none">
          <Switch
            checked={changedOnly}
            onCheckedChange={setChangedOnly}
            aria-label={m.runs_files_changed_only()}
          />
          {m.runs_files_changed_only()}
        </label>
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
            onPick={setPicked}
          />
        </div>
      )}
      {picked ? (
        <FileViewer id={id} row={picked} />
      ) : (
        roots.length > 0 && (
          <p className="text-sm text-surface-400 select-none">{m.runs_file_pick_hint()}</p>
        )
      )}
    </div>
  );
}
