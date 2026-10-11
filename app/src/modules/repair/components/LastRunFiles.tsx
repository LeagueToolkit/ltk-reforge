import { type ReactNode, useState } from "react";

import { TabPanel, Tabs } from "@/components";
import { m } from "@/i18n";
import type { RunView, StageName } from "@/lib/bindings.gen";
import {
  FileJumpContext,
  type FilesAt,
  FilesBrowser,
  heldStages,
  type Picked,
  useFilesAt,
} from "@/modules/files";

import { lastFileQuery, lastFilesQuery, useLastFiles } from "../api/run";

interface LastRunFilesProps {
  /** The stages the run holds files for, in order. */
  stages: StageName[];
  /** Where the browser is, which the page keeps. */
  at: FilesAt;
  onStage: (stage: StageName) => void;
  onPick: (picked: Picked | null) => void;
}

/**
 * The last run's files at each stage it holds files for, each file marked against the stage
 * before, kept by the store or not.
 */
export function LastRunFiles({ stages, at, onStage, onPick }: LastRunFilesProps) {
  const stage = at.stage && stages.includes(at.stage) ? at.stage : stages[stages.length - 1];
  const files = useLastFiles(stage);
  return (
    <FilesBrowser
      files={files}
      stages={stages}
      stage={stage}
      onStage={onStage}
      fileQuery={(row, entry) => lastFileQuery(row.content, entry)}
      picked={at.picked}
      onPick={onPick}
    />
  );
}

/**
 * The last run's files browser's place and the jump to a value's bin, for a page whose files
 * tab `showFiles` shows. No jump while the run holds no files at `build`.
 */
export function useLastRunFiles(stages: StageName[], showFiles: () => void) {
  return useFilesAt(stages.includes("build") ? lastFilesQuery("build") : null, showFiles);
}

/** The tabs of a run that ended. */
type Tab = "result" | "files";

interface EndedTabsProps {
  /** The run that ended. Null while one is under way, which holds no files yet. */
  view: RunView | null;
  /** Its result. */
  children: ReactNode;
}

/**
 * A run's result, and beside it, once the run ended with files, its files in a second tab: the
 * simple and the guided view's, as the Runs page shows a kept run's. A value of a finding
 * opens in its bin there.
 */
export function EndedTabs({ view, children }: EndedTabsProps) {
  const [tab, setTab] = useState<Tab>("result");
  const stages = heldStages(view);
  const { at, setStage, setPicked, jump } = useLastRunFiles(stages, () => setTab("files"));
  if (stages.length === 0) {
    return children;
  }
  const tabs = [
    { value: "result" as const, label: m.runs_tab_result() },
    { value: "files" as const, label: m.runs_tab_files() },
  ];
  return (
    <FileJumpContext value={jump}>
      <Tabs tabs={tabs} value={tab} onChange={setTab} aria-label={m.runs_tabs_label()}>
        <TabPanel value="result">{children}</TabPanel>
        <TabPanel value="files">
          <LastRunFiles stages={stages} at={at} onStage={setStage} onPick={setPicked} />
        </TabPanel>
      </Tabs>
    </FileJumpContext>
  );
}
