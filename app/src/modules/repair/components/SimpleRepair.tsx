import { WrenchIcon } from "@phosphor-icons/react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import type { RunRequest } from "@/lib/bindings.gen";
import { useInputs, useInputsGate } from "@/modules/inputs";

import { useRunMod } from "../api/run";
import { runSettings, STANDARD, useRepairStore } from "../state/repairStore";
import { ChangesList } from "./ChangesList";
import { DecisionsView } from "./DecisionsView";
import { LiveProgress } from "./LiveProgress";
import { useOutputFolder } from "./ModCard";
import { RunDetails } from "./RunDetails";
import { RunFindings } from "./RunFindings";
import { RunSummary } from "./RunSummary";

/** The write is refused while the run holds an error, so every error is seen before it. */
const ERRORS_BLOCK = ["severity=error"];

/**
 * The simple view (#ADR-0050): one button runs every stage with the standard settings and
 * writes the package. A run with an error is not written; the view shows the errors, and a
 * second button writes it anyway (`docs/design/ROADMAP-UX.md`, "Two views of one run").
 */
export function SimpleRepair() {
  const archive = useRepairStore((state) => state.archive);
  const kind = useRepairStore((state) => state.kind);
  const last = useRepairStore((state) => state.last);
  const live = useRepairStore((state) => state.live);
  const out = useOutputFolder();
  const inputs = useInputs();
  const run = useRunMod();
  const { gate, dialog } = useInputsGate();

  const start = (block: string[]) => {
    if (!archive || !out) {
      return;
    }
    const request: RunRequest = {
      kind,
      archive,
      through: "write",
      settings: runSettings({ ...STANDARD, block }),
      out,
    };
    gate(() => run.mutate(request));
  };
  const stages = kind === "port" ? PORT_STAGES : REPAIR_STAGES;

  return (
    <div className="flex flex-col gap-6" data-ui="SimpleRepair">
      <div className="flex items-center justify-between gap-4">
        <p className="text-sm text-surface-400 select-none">
          {kind === "port" ? m.run_simple_port_description() : m.repair_run_description()}
        </p>
        <Button
          variant="filled"
          left={<WrenchIcon weight="bold" />}
          loading={run.isPending}
          disabled={!archive || !out || inputs.isPending}
          onClick={() => start(ERRORS_BLOCK)}
        >
          {kind === "port" ? m.run_kind_port_label() : m.repair_run_action()}
        </Button>
      </div>
      {dialog}
      {live && <LiveProgress live={live} stages={[...stages]} />}
      {run.isError && <FailureNote failure={run.error} />}
      {last && !live && (
        <>
          <RunSummary view={last} onWriteAnyway={() => start([])} writing={run.isPending} />
          <RunDetails lines={last.log} />
          <RunFindings findings={last.findings} />
          {last.targets && <DecisionsView targets={last.targets} />}
          <ChangesList changes={last.changes} />
        </>
      )}
    </div>
  );
}

export const REPAIR_STAGES = ["read", "load", "fix", "build", "check", "write"] as const;
export const PORT_STAGES = [
  "read",
  "load",
  "fix",
  "facts",
  "decide",
  "plan",
  "build",
  "check",
  "write",
] as const;
