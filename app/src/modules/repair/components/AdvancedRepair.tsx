import { PlayIcon } from "@phosphor-icons/react";
import { useState } from "react";

import {
  Button,
  FailureNote,
  SectionCard,
  SegmentedControl,
  Switch,
  TabPanel,
  Tabs,
} from "@/components";
import { m, stageLabel } from "@/i18n";
import type { PolicyView, RunView, StageName } from "@/lib/bindings.gen";
import { ChangesExplorer } from "@/modules/changes";
import { FileJumpContext, heldStages } from "@/modules/files";
import { FindingList, worstFirst } from "@/modules/findings";
import { useCheckCatalog, useFixupCatalog } from "@/modules/fixups";
import { useInputs, useInputsGate } from "@/modules/inputs";

import { useJobRunning, useRunMod, useRunRunning, useRunStages } from "../api/run";
import { REPAIR, runSettings, STANDARD, toggled, useRepairStore } from "../state/repairStore";
import { BlockPicker } from "./BlockPicker";
import { DecisionsView } from "./DecisionsView";
import { GuidedSettingsCard } from "./GuidedSettingsCard";
import { LastRunFiles, useLastRunFiles } from "./LastRunFiles";
import { LiveProgress } from "./LiveProgress";
import { useOutputFolder } from "./ModCard";
import { PolicyCard } from "./PolicyCard";
import { RulesCard } from "./RulesCard";
import { fixupDetail } from "./RunFindings";
import { liveResults, RunResults } from "./RunResults";
import { RunSummary } from "./RunSummary";
import { StageStrip } from "./StageStrip";
import { StopButton } from "./StopButton";
import { type ToggleItem, ToggleList } from "./ToggleList";

/** The advanced view's tabs. */
type Tab = "result" | "files" | "settings" | "toggles" | "rules";

/** The run bar: the last stage, the presets, and the one Run button every tab shares. */
function RunBar({
  stages,
  onRun,
  running,
}: {
  stages: readonly StageName[];
  onRun: () => void;
  running: boolean;
}) {
  const advanced = useRepairStore((state) => state.advanced);
  const setAdvanced = useRepairStore((state) => state.setAdvanced);
  const applyPreset = useRepairStore((state) => state.applyPreset);
  const archive = useRepairStore((state) => state.archive);
  const inputs = useInputs();
  // A batch under way holds the run job.
  const busy = useJobRunning();
  const options = stages.map((stage) => ({ value: stage, label: stageLabel(stage) }));
  const through = stages.includes(advanced.through) ? advanced.through : "write";

  return (
    <div className="flex flex-wrap items-center gap-3" data-ui="RunBar">
      {running ? (
        <StopButton />
      ) : (
        <Button
          variant="filled"
          left={<PlayIcon weight="bold" />}
          disabled={!archive || inputs.isPending || busy}
          onClick={onRun}
        >
          {m.advanced_run_action()}
        </Button>
      )}
      <span className="text-sm text-surface-400 select-none">{m.advanced_through_label()}</span>
      <SegmentedControl
        options={options}
        value={through}
        onChange={(next) => setAdvanced({ through: next })}
        aria-label={m.advanced_through_label()}
      />
      <div className="ml-auto flex gap-1">
        <Button size="xs" variant="ghost" onClick={() => applyPreset(STANDARD)}>
          {m.advanced_reset_action()}
        </Button>
        <Button size="xs" variant="ghost" onClick={() => applyPreset(REPAIR)}>
          {m.advanced_repair_preset_action()}
        </Button>
      </div>
    </div>
  );
}

/** The run settings: the stale copies, the DDS textures, and what refuses the write. */
function SettingsCard() {
  const advanced = useRepairStore((state) => state.advanced);
  const setAdvanced = useRepairStore((state) => state.setAdvanced);

  return (
    <SectionCard title={m.advanced_settings_title()}>
      {advanced.kind === "repair" && (
        <p
          className="rounded-md border border-warning/40 bg-warning/10 p-2 text-xs text-surface-200 select-none"
          data-ui="SettingsCard:repair"
        >
          {m.advanced_repair_preset_note()}
        </p>
      )}
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-sm text-surface-100 select-none">{m.advanced_keep_stale_label()}</p>
          <p className="text-xs text-surface-400 select-none">
            {m.advanced_keep_stale_description()}
          </p>
        </div>
        <Switch
          checked={advanced.keepStale}
          onCheckedChange={(keepStale) => setAdvanced({ keepStale })}
          aria-label={m.advanced_keep_stale_label()}
        />
      </div>
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-sm text-surface-100 select-none">{m.advanced_keep_dds_label()}</p>
          <p className="text-xs text-surface-400 select-none">
            {m.advanced_keep_dds_description()}
          </p>
        </div>
        <Switch
          checked={advanced.keepDds}
          onCheckedChange={(keepDds) => setAdvanced({ keepDds })}
          aria-label={m.advanced_keep_dds_label()}
        />
      </div>
      <BlockPicker block={advanced.block} onChange={(block) => setAdvanced({ block })} />
    </SectionCard>
  );
}

/** What one stage gave: its result where the views draw one, and the findings it raised. */
function StageDetail({
  view,
  stage,
  onPolicy,
}: {
  view: RunView;
  stage: StageName;
  onPolicy: (policy: PolicyView) => void;
}) {
  const state = view.stages.find((s) => s.stage === stage)?.state;
  const findings = worstFirst(view.findings.filter((finding) => finding.stage === stage));
  return (
    <div className="flex flex-col gap-4" data-ui="StageDetail">
      <SectionCard title={stageLabel(stage)} description={m[`stage.${stage}.description`]()}>
        {state === "not-run" && (
          <p className="text-sm text-surface-400 select-none">{m.advanced_stage_not_run()}</p>
        )}
        {stage === "build" && view.package && (
          <p className="text-sm text-surface-200">
            {m.advanced_package_label({
              name: view.package.name,
              version: view.package.version,
              chunks: view.package.chunks,
              patches: view.package.patches,
            })}
          </p>
        )}
        {stage === "write" && view.written && (
          <p className="font-mono text-xs break-all text-surface-200">
            {m.advanced_written_label({ path: view.written })}
          </p>
        )}
        <FindingList
          findings={findings}
          empty={m.advanced_stage_findings_empty()}
          detailOf={fixupDetail(view.changes, view.objects)}
          names={view.names}
        />
      </SectionCard>
      {stage === "fix" && state === "ran" && (
        <ChangesExplorer changes={view.changes} objects={view.objects} findings={view.findings} />
      )}
      {stage === "decide" && view.targets && (
        <DecisionsView targets={view.targets} onPolicy={onPolicy} />
      )}
    </div>
  );
}

/** The fixups and the checks, each turned on and off from the standard set. */
function Toggles() {
  const advanced = useRepairStore((state) => state.advanced);
  const setAdvanced = useRepairStore((state) => state.setAdvanced);
  const fixups = useFixupCatalog();
  const checks = useCheckCatalog();

  const fixupItems: ToggleItem[] = (fixups.data ?? []).map((fixup) => {
    const on = fixup.default
      ? !advanced.fixupsOff.includes(fixup.id)
      : advanced.fixupsOn.includes(fixup.id);
    return {
      id: fixup.id,
      title: fixup.title,
      records: fixup.records,
      on,
      changed: on !== fixup.default,
    };
  });
  const toggleFixup = (id: string) => {
    const fixup = fixups.data?.find((entry) => entry.id === id);
    if (fixup?.default) {
      setAdvanced({ fixupsOff: toggled(advanced.fixupsOff, id) });
    } else {
      setAdvanced({ fixupsOn: toggled(advanced.fixupsOn, id) });
    }
  };

  const checkItems: ToggleItem[] = (checks.data ?? []).map((check) => {
    const on = !advanced.checksOff.includes(check.id);
    const records = check.record ? [check.record] : [];
    return { id: check.id, title: check.title, records, on, changed: !on };
  });

  return (
    <>
      <ToggleList
        title={m.fixups_list_title()}
        description={m.advanced_fixups_description()}
        items={fixupItems}
        onToggle={toggleFixup}
      />
      <ToggleList
        title={m.advanced_checks_title()}
        description={m.advanced_checks_description()}
        items={checkItems}
        onToggle={(id) => setAdvanced({ checksOff: toggled(advanced.checksOff, id) })}
      />
    </>
  );
}

/**
 * The advanced view (#ADR-0050): the run's stages in order, a run up to one of them, and the
 * settings the library has, under four tabs with one Run bar over them. Each run is a new run
 * from the archive, and starting one shows the result.
 */
export function AdvancedRepair() {
  const archive = useRepairStore((state) => state.archive);
  const advanced = useRepairStore((state) => state.advanced);
  const setAdvanced = useRepairStore((state) => state.setAdvanced);
  const kind = advanced.kind;
  const last = useRepairStore((state) => state.last);
  const live = useRepairStore((state) => state.live);
  const out = useOutputFolder();
  const run = useRunMod();
  const running = useRunRunning();
  const { gate, dialog } = useInputsGate();
  const [selected, setSelected] = useState<StageName | null>(null);
  const [tab, setTab] = useState<Tab>("result");
  const stages = useRunStages(kind);

  const start = (settings = advanced) => {
    if (!archive) {
      return;
    }
    const through = stages.includes(settings.through) ? settings.through : "write";
    setSelected(null);
    setTab("result");
    gate(() =>
      run.mutate({
        kind: settings.kind,
        archive,
        through,
        settings: runSettings(settings),
        out: through === "write" ? out : null,
      }),
    );
  };
  const runWithPolicy = (policy: PolicyView) => {
    const next = { ...advanced, target: policy.target, animReuse: policy.anim_reuse };
    setAdvanced({ target: policy.target, animReuse: policy.anim_reuse });
    start(next);
  };
  const lastRan = last?.stages.filter((stage) => stage.state !== "not-run").at(-1)?.stage;
  const shown = selected ?? lastRan ?? "read";
  const togglesChanged =
    advanced.fixupsOff.length + advanced.fixupsOn.length + advanced.checksOff.length;

  // The run under way holds no files yet.
  const held = live ? [] : heldStages(last);
  const files = useLastRunFiles(held, () => setTab("files"));
  const tabs = [
    { value: "result" as const, label: m.advanced_tab_result() },
    ...(held.length > 0 ? [{ value: "files" as const, label: m.runs_tab_files() }] : []),
    { value: "settings" as const, label: m.advanced_tab_settings() },
    { value: "toggles" as const, label: m.advanced_tab_toggles(), count: togglesChanged },
    { value: "rules" as const, label: m.advanced_tab_rules(), count: advanced.rules.length },
  ];

  return (
    <div className="flex flex-col gap-6" data-ui="AdvancedRepair">
      <RunBar stages={stages} onRun={() => start()} running={running} />
      {dialog}
      <FileJumpContext value={files.jump}>
        <Tabs tabs={tabs} value={tab} onChange={setTab} aria-label={m.advanced_tabs_label()}>
          <TabPanel value="result">
            {live && <LiveProgress live={live} stages={stages} />}
            {live && <RunResults {...liveResults(live)} />}
            {run.isError && <FailureNote failure={run.error} />}
            {last && !live && (
              <>
                <RunSummary view={last} />
                <StageStrip
                  stages={last.stages}
                  findings={last.findings}
                  selected={shown}
                  onSelect={setSelected}
                />
                <StageDetail view={last} stage={shown} onPolicy={runWithPolicy} />
              </>
            )}
            {!last && !live && !run.isError && (
              <p className="text-sm text-surface-400 select-none" data-ui="AdvancedRepair:empty">
                {m.advanced_no_run_yet()}
              </p>
            )}
          </TabPanel>
          {held.length > 0 && (
            <TabPanel value="files">
              <LastRunFiles
                stages={held}
                at={files.at}
                onStage={files.setStage}
                onPick={files.setPicked}
              />
            </TabPanel>
          )}
          <TabPanel value="settings">
            <SettingsCard />
            {kind === "port" && <PolicyCard />}
            <GuidedSettingsCard />
          </TabPanel>
          <TabPanel value="toggles">
            <Toggles />
          </TabPanel>
          <TabPanel value="rules">
            <RulesCard />
          </TabPanel>
        </Tabs>
      </FileJumpContext>
    </div>
  );
}
