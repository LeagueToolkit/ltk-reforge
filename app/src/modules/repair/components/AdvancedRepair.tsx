import { PlayIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { Button, FailureNote, SectionCard, SegmentedControl, Switch } from "@/components";
import { m } from "@/i18n";
import type { PolicyView, RunView, StageName } from "@/lib/bindings.gen";
import { ChangesExplorer } from "@/modules/changes";
import { FindingList, worstFirst } from "@/modules/findings";
import { useCheckCatalog, useFixupCatalog } from "@/modules/fixups";
import { useInputs, useInputsGate } from "@/modules/inputs";

import { useRunMod, useRunStages } from "../api/run";
import { REPAIR, runSettings, STANDARD, toggled, useRepairStore } from "../state/repairStore";
import { DecisionsView } from "./DecisionsView";
import { LiveProgress } from "./LiveProgress";
import { useOutputFolder } from "./ModCard";
import { PolicyCard } from "./PolicyCard";
import { RulesCard } from "./RulesCard";
import { RunDetails } from "./RunDetails";
import { fixupDetail } from "./RunFindings";
import { liveResults, RunResults } from "./RunResults";
import { RunSummary } from "./RunSummary";
import { StageStrip, stageLabel } from "./StageStrip";
import { type ToggleItem, ToggleList } from "./ToggleList";

/** The run settings: the last stage, the stale copies, and what refuses the write. */
function SettingsCard({
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
  const options = stages.map((stage) => ({ value: stage, label: stageLabel(stage) }));
  const through = stages.includes(advanced.through) ? advanced.through : "write";

  return (
    <SectionCard
      title={m.advanced_settings_title()}
      action={
        <div className="flex gap-1">
          <Button size="xs" variant="ghost" onClick={() => applyPreset(STANDARD)}>
            {m.advanced_reset_action()}
          </Button>
          <Button size="xs" variant="ghost" onClick={() => applyPreset(REPAIR)}>
            {m.advanced_repair_preset_action()}
          </Button>
        </div>
      }
    >
      {advanced.kind === "repair" && (
        <p
          className="rounded-md border border-warning/40 bg-warning/10 p-2 text-xs text-surface-200 select-none"
          data-ui="SettingsCard:repair"
        >
          {m.advanced_repair_preset_note()}
        </p>
      )}
      <div className="flex flex-col gap-2">
        <p className="text-sm text-surface-100 select-none">{m.advanced_through_label()}</p>
        <SegmentedControl
          options={options}
          value={through}
          onChange={(next) => setAdvanced({ through: next })}
          aria-label={m.advanced_through_label()}
        />
      </div>
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
      <label className="flex flex-col gap-1">
        <span className="text-sm text-surface-100 select-none">{m.advanced_block_label()}</span>
        <span className="text-xs text-surface-400 select-none">
          {m.advanced_block_description()}
        </span>
        <textarea
          rows={2}
          value={advanced.block.join("\n")}
          onChange={(event) => setAdvanced({ block: event.target.value.split("\n") })}
          className="rounded-md border border-surface-600 bg-surface-950/40 p-2 font-mono text-xs text-surface-100 focus-visible:outline-2 focus-visible:outline-accent-500"
        />
      </label>
      <div className="flex justify-end">
        <Button
          variant="filled"
          left={<PlayIcon weight="bold" />}
          loading={running}
          disabled={!archive || inputs.isPending}
          onClick={onRun}
        >
          {m.advanced_run_action()}
        </Button>
      </div>
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
      meta: fixup.records.join(" "),
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
    return { id: check.id, title: check.title, meta: check.record ?? "", on, changed: !on };
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
 * settings the library has. Each run is a new run from the archive.
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
  const { gate, dialog } = useInputsGate();
  const [selected, setSelected] = useState<StageName | null>(null);
  const stages = useRunStages(kind);

  const start = (settings = advanced) => {
    if (!archive) {
      return;
    }
    const through = stages.includes(settings.through) ? settings.through : "write";
    setSelected(null);
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

  return (
    <div className="flex flex-col gap-6" data-ui="AdvancedRepair">
      <SettingsCard stages={stages} onRun={() => start()} running={run.isPending} />
      {dialog}
      {live && <LiveProgress live={live} stages={stages} />}
      {live && <RunResults {...liveResults(live)} />}
      {run.isError && <FailureNote failure={run.error} />}
      {last && !live && (
        <>
          <RunSummary view={last} />
          <RunDetails lines={last.log} />
          <StageStrip
            stages={last.stages}
            findings={last.findings}
            selected={shown}
            onSelect={setSelected}
          />
          <StageDetail view={last} stage={shown} onPolicy={runWithPolicy} />
        </>
      )}
      {kind === "port" && <PolicyCard />}
      <RulesCard />
      <Toggles />
    </div>
  );
}
