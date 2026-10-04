import { WrenchIcon } from "@phosphor-icons/react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import type { RunRequest } from "@/lib/bindings.gen";
import { useInputs } from "@/modules/inputs";

import { useRunRepair } from "../api/run";
import { useRepairStore } from "../state/repairStore";
import { ChangesList } from "./ChangesList";
import { useOutputFolder } from "./ModCard";
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
  const last = useRepairStore((state) => state.last);
  const out = useOutputFolder();
  const inputs = useInputs();
  const run = useRunRepair();

  const request = (block: string[]): RunRequest | null =>
    archive && out ? { archive, through: "write", settings: { block }, out } : null;
  const start = (block: string[]) => {
    const asked = request(block);
    if (asked) {
      run.mutate(asked);
    }
  };

  return (
    <div className="flex flex-col gap-6" data-ui="SimpleRepair">
      <div className="flex items-center justify-between gap-4">
        <p className="text-sm text-surface-400 select-none">{m.repair_run_description()}</p>
        <Button
          variant="filled"
          left={<WrenchIcon weight="bold" />}
          loading={run.isPending}
          disabled={!archive || !out || !inputs.isSuccess}
          onClick={() => start(ERRORS_BLOCK)}
        >
          {run.isPending ? m.repair_running_label() : m.repair_run_action()}
        </Button>
      </div>
      {inputs.isError && <FailureNote failure={inputs.error} />}
      {run.isError && <FailureNote failure={run.error} />}
      {last && (
        <>
          <RunSummary view={last} onWriteAnyway={() => start([])} writing={run.isPending} />
          <RunFindings findings={last.findings} />
          <ChangesList changes={last.changes} />
        </>
      )}
    </div>
  );
}
