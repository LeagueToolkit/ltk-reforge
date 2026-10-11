import { MagnifyingGlassIcon, PlayIcon } from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useCallback, useState } from "react";

import { Button, FailureNote, SectionCard, Spinner, TabPanel, Tabs } from "@/components";
import { m } from "@/i18n";
import type { ExploreView } from "@/lib/bindings.gen";
import { FilesBrowser } from "@/modules/files";
import { EXPLORED, FindingList, RunRefContext, worstFirst } from "@/modules/findings";
import { StaleInputsLink } from "@/modules/inputs";
import {
  chooseArchive,
  ModFace,
  peekQuery,
  useArchiveDrop,
  useRepairStore,
  useRunRunning,
  useTakeArchive,
} from "@/modules/repair";

import { exploreFileQuery, useExploreFiles, useExploreMod, useExploreRun } from "../api/explore";
import { haltingFinding } from "../model";
import { useModStore } from "../state/modStore";
import { Overview } from "./Overview";

/** The tabs of a mod explored. */
type Tab = "overview" | "files" | "findings";

/** The mod explored's files at `load`, each with whose bytes it holds. */
function FilesTab() {
  const files = useExploreFiles(true);
  return (
    <FilesBrowser
      files={files}
      stages={["load"]}
      stage="load"
      onStage={() => undefined}
      fileQuery={(row) => exploreFileQuery(row.content)}
      marked={false}
    />
  );
}

/** What ended the explore's run early: the halting finding, or an input that did not answer. */
function Ended({ view }: { view: ExploreView }) {
  const halting = haltingFinding(view);
  if (!halting && !view.fault) {
    return null;
  }
  return (
    <SectionCard title={m.mod_halted_title()} description={m.mod_halted_description()}>
      {halting && <FindingList findings={[halting]} empty="" names={view.names} />}
      {view.fault && (
        <p role="alert" className="text-sm break-words text-danger-text">
          {m.mod_fault_label({ detail: view.fault })}
        </p>
      )}
    </SectionCard>
  );
}

/** The tabs of a mod explored: the load report, its files and its findings. */
function Explored({ view }: { view: ExploreView }) {
  const [tab, setTab] = useState<Tab>("overview");
  // `load` ended when it dated the mod, and only then does it name files.
  const loaded = view.dating != null;
  const tabs = [
    { value: "overview" as const, label: m.mod_tab_overview() },
    ...(loaded ? [{ value: "files" as const, label: m.mod_tab_files() }] : []),
    { value: "findings" as const, label: m.mod_tab_findings(), count: view.findings.length },
  ];
  return (
    <Tabs tabs={tabs} value={tab} onChange={setTab} aria-label={m.mod_tabs_label()}>
      <TabPanel value="overview">
        <Overview view={view} />
      </TabPanel>
      {loaded && (
        <TabPanel value="files">
          <FilesTab />
        </TabPanel>
      )}
      <TabPanel value="findings">
        <RunRefContext value={EXPLORED}>
          <FindingList
            findings={worstFirst(view.findings)}
            empty={m.mod_findings_empty()}
            names={view.names}
          />
        </RunRefContext>
      </TabPanel>
    </Tabs>
  );
}

/**
 * What an archive holds before a run (#ADR-0091): the mod read alone as soon as it is picked,
 * and once explored, the load report rendered. The page adds no rule about a mod and keeps
 * nothing in the store.
 */
export function ModPage() {
  const archive = useModStore((state) => state.archive);
  const explored = useModStore((state) => state.explored);
  const setArchive = useModStore((state) => state.setArchive);
  const takeRun = useRepairStore((state) => state.takeRun);
  const navigate = useNavigate();
  const running = useRunRunning();
  const explore = useExploreMod();
  const runThis = useExploreRun();
  const peek = useQuery(peekQuery(archive));
  const busy = running || explore.isPending;
  const take = useTakeArchive(setArchive);
  // The page explores one mod: the first archive dropped.
  const onDrop = useCallback(
    ([first]: string[]) => {
      if (!busy && first) {
        void take(first);
      }
    },
    [busy, take],
  );
  const over = useArchiveDrop(onDrop);
  const view = explored?.view ?? null;

  const choose = async () => {
    const picked = await chooseArchive();
    if (picked) {
      await take(picked);
    }
  };
  // As the Runs page's Run again: the run page takes the mod and its settings, and nothing
  // runs until the person presses Run.
  const runIt = () =>
    runThis.mutate(undefined, {
      onSuccess: (again) => {
        if (again?.archive_present) {
          takeRun(again);
          void navigate({ to: "/" });
        }
      },
    });
  const absent = runThis.data != null && !runThis.data.archive_present;

  return (
    <div className="flex flex-col gap-6" data-ui="ModPage">
      <div className="flex flex-col gap-1">
        <div className="flex items-center gap-3">
          <h1 className="font-display text-xl font-semibold text-surface-50">
            {m.mod_page_title()}
          </h1>
          <StaleInputsLink />
        </div>
        <p className="text-sm text-surface-400">{m.mod_page_description()}</p>
      </div>
      <SectionCard title={m.mod_card_title()}>
        <ModFace
          archive={archive}
          meta={peek.data?.meta ?? view?.meta}
          thumbnail={peek.data?.thumbnail ?? view?.thumbnail}
          characters={peek.data?.characters ?? []}
          failure={peek.isError ? peek.error : undefined}
          over={over && !busy}
          actions={
            <>
              <Button size="sm" variant="outline" disabled={busy} onClick={() => void choose()}>
                {m.repair_mod_choose_action()}
              </Button>
              <Button
                size="sm"
                variant="filled"
                left={<MagnifyingGlassIcon weight="bold" />}
                disabled={!archive || running}
                loading={explore.isPending}
                title={m.mod_explore_description()}
                onClick={() => archive && explore.mutate(archive)}
              >
                {m.mod_explore_action()}
              </Button>
              <Button
                size="sm"
                variant="outline"
                left={<PlayIcon weight="bold" />}
                disabled={!view || busy}
                loading={runThis.isPending}
                title={m.mod_run_description()}
                onClick={runIt}
              >
                {m.mod_run_action()}
              </Button>
            </>
          }
        />
      </SectionCard>
      {explore.isPending && (
        <p className="flex items-center gap-2 text-sm text-surface-300 select-none">
          <Spinner size="sm" />
          {m.mod_exploring()}
        </p>
      )}
      {explore.isError && <FailureNote failure={explore.error} />}
      {runThis.isError && <FailureNote failure={runThis.error} />}
      {absent && (
        <p role="alert" className="text-sm text-danger-text">
          {m.mod_run_missing()}
        </p>
      )}
      {/* Hidden while an explore runs, so the next one opens at its overview. */}
      {view && !explore.isPending && (
        <>
          <Ended view={view} />
          <Explored view={view} />
        </>
      )}
      {!view && archive && !explore.isPending && (
        <p className="text-sm text-surface-400 select-none">{m.mod_explore_hint()}</p>
      )}
    </div>
  );
}
