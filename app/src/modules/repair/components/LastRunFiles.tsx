import { type ReactNode, useState } from "react";

import { TabPanel, Tabs } from "@/components";
import { m } from "@/i18n";
import type { RunView, StageName } from "@/lib/bindings.gen";
import { FilesBrowser, heldStages } from "@/modules/files";

import { lastFileQuery, useLastFiles } from "../api/run";

interface LastRunFilesProps {
  /** The stages the run holds files for, in order. */
  stages: StageName[];
}

/**
 * The last run's files at each stage it holds files for, each file marked against the stage
 * before, kept by the store or not.
 */
export function LastRunFiles({ stages }: LastRunFilesProps) {
  const [picked, setPicked] = useState<StageName | null>(null);
  const stage = picked && stages.includes(picked) ? picked : stages[stages.length - 1];
  const files = useLastFiles(stage);
  return (
    <FilesBrowser
      files={files}
      stages={stages}
      stage={stage}
      onStage={setPicked}
      fileQuery={(row) => lastFileQuery(row.content)}
    />
  );
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
 * simple and the guided view's, as the Runs page shows a kept run's.
 */
export function EndedTabs({ view, children }: EndedTabsProps) {
  const [tab, setTab] = useState<Tab>("result");
  const stages = heldStages(view);
  if (stages.length === 0) {
    return children;
  }
  const tabs = [
    { value: "result" as const, label: m.runs_tab_result() },
    { value: "files" as const, label: m.runs_tab_files() },
  ];
  return (
    <Tabs tabs={tabs} value={tab} onChange={setTab} aria-label={m.runs_tabs_label()}>
      <TabPanel value="result">{children}</TabPanel>
      <TabPanel value="files">
        <LastRunFiles stages={stages} />
      </TabPanel>
    </Tabs>
  );
}
