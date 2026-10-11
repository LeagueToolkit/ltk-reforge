import { CaretDownIcon, TrashIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { Button, FailureNote, Menu } from "@/components";
import { m } from "@/i18n";
import type { RunRow } from "@/lib/bindings.gen";
import { ConfirmRemove, diskSize } from "@/modules/inputs";

import { useClearModIngest, useModIngestBytes, useRemoveModRuns, useRemoveRun } from "../api/runs";

/** What the menu asks about before it removes. */
type Asking = "run" | "mod" | "ingest" | null;

interface RemoveMenuProps {
  row: RunRow;
  /** How many kept runs are of the run's mod, it among them. */
  modRuns: number;
  /** Called once the run is gone, by itself or with its mod's. */
  onRemoved: () => void;
}

/**
 * What a kept run's Remove offers: the run, every run of its mod, or the mod's archive data in
 * the ingest tier, each asked about with what it frees.
 */
export function RemoveMenu({ row, modRuns, onRemoved }: RemoveMenuProps) {
  const removeRun = useRemoveRun();
  const removeMod = useRemoveModRuns();
  const clearIngest = useClearModIngest();
  const [asking, setAsking] = useState<Asking>(null);
  const ingest = useModIngestBytes(row.sha256, asking === "ingest");
  const pending = removeRun.isPending || removeMod.isPending || clearIngest.isPending;
  const failure = removeRun.error ?? removeMod.error ?? clearIngest.error;
  const close = () => setAsking(null);

  return (
    <>
      <Menu
        trigger={
          <Button
            size="sm"
            variant="ghost"
            className="ml-auto"
            left={<TrashIcon weight="bold" />}
            right={<CaretDownIcon weight="bold" />}
            loading={pending}
          >
            {m.runs_remove_action()}
          </Button>
        }
        items={[
          { label: m.runs_remove_run_action(), danger: true, onSelect: () => setAsking("run") },
          { label: m.runs_remove_mod_action(), danger: true, onSelect: () => setAsking("mod") },
          {
            label: m.runs_clear_ingest_action(),
            danger: true,
            disabled: !row.archive_ingested,
            onSelect: () => setAsking("ingest"),
          },
        ]}
      />
      {failure && (
        <div className="basis-full">
          <FailureNote failure={failure} />
        </div>
      )}
      <ConfirmRemove
        open={asking === "run"}
        onClose={close}
        title={m.runs_remove_title()}
        description={m.runs_remove_description()}
        onConfirm={() => removeRun.mutate(row.id, { onSuccess: onRemoved })}
      />
      <ConfirmRemove
        open={asking === "mod"}
        onClose={close}
        title={m.runs_remove_mod_title()}
        description={m.runs_remove_mod_description({ count: modRuns })}
        onConfirm={() => removeMod.mutate(row.sha256, { onSuccess: onRemoved })}
      />
      <ConfirmRemove
        open={asking === "ingest"}
        onClose={close}
        title={m.runs_clear_ingest_title()}
        description={m.runs_clear_ingest_description({
          bytes: ingest.data == null ? "…" : diskSize(ingest.data),
        })}
        onConfirm={() => clearIngest.mutate(row.sha256)}
      />
    </>
  );
}
