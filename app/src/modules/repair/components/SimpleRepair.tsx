import { WrenchIcon } from "@phosphor-icons/react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import type { RunRequest } from "@/lib/bindings.gen";
import { useInputs, useInputsGate } from "@/modules/inputs";

import { useJobRunning, useRunMod, useRunRunning, useRunStages } from "../api/run";
import { runSettings, STANDARD, useRepairStore } from "../state/repairStore";
import { EndedTabs } from "./LastRunFiles";
import { LiveProgress } from "./LiveProgress";
import { useOutputFolder, usePackageFormat } from "./ModCard";
import { PackageFormatMenu } from "./PackageFormatMenu";
import { resultsOf, RunResults } from "./RunResults";
import { RunSummary } from "./RunSummary";
import { StopButton } from "./StopButton";

/** The write is refused while the run holds an error, so every error is seen before it. */
const ERRORS_BLOCK = ["severity=error"];

/**
 * The simple view (#ADR-0050): one button ports the mod through every stage with the standard
 * settings (#ADR-0052) and writes the package. A run with an error is not written; the view
 * shows the errors, and a second button writes it anyway (`docs/design/ROADMAP-UX.md`, "Two
 * views of one run").
 */
export function SimpleRepair() {
  const archive = useRepairStore((state) => state.archive);
  const kind = STANDARD.kind;
  const last = useRepairStore((state) => state.last);
  const live = useRepairStore((state) => state.live);
  const out = useOutputFolder();
  const format = usePackageFormat();
  const inputs = useInputs();
  const run = useRunMod();
  const running = useRunRunning();
  // A batch under way holds the run job.
  const busy = useJobRunning();
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
      format,
    };
    gate(() => run.mutate(request));
  };
  const stages = useRunStages(kind);
  const results = resultsOf(live, last);

  return (
    <div className="flex flex-col gap-6" data-ui="SimpleRepair">
      <div className="flex items-center justify-between gap-4">
        <p className="text-sm text-surface-400 select-none">{m.run_simple_port_description()}</p>
        <div className="flex shrink-0 items-center gap-2">
          {!running && <PackageFormatMenu />}
          {running ? (
            <StopButton />
          ) : (
            <Button
              variant="filled"
              left={<WrenchIcon weight="bold" />}
              disabled={!archive || !out || inputs.isPending || busy}
              onClick={() => start(ERRORS_BLOCK)}
            >
              {m.run_kind_port_label()}
            </Button>
          )}
        </div>
      </div>
      {dialog}
      {live && <LiveProgress live={live} stages={stages} />}
      {run.isError && <FailureNote failure={run.error} />}
      {last && !live && (
        <RunSummary view={last} onWriteAnyway={() => start([])} writing={running} />
      )}
      {results && (
        <EndedTabs view={live ? null : last}>
          <RunResults {...results} />
        </EndedTabs>
      )}
    </div>
  );
}
