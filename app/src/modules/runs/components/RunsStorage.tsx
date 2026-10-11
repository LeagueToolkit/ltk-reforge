import { BroomIcon, TrashIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import { ConfirmRemove, diskSize } from "@/modules/inputs";

import { useClearUnusedIngest, useRemoveAllRuns, useStoreView } from "../api/runs";

/** What the storage group asks about before it removes. */
type Asking = "runs" | "unused" | null;

/** The store of runs as the settings page's storage group shows it: its counts, its bytes. */
export function RunsStorage() {
  const view = useStoreView();
  const remove = useRemoveAllRuns();
  const clearUnused = useClearUnusedIngest();
  const [asking, setAsking] = useState<Asking>(null);
  const store = view.data;
  const failure = remove.error ?? clearUnused.error;

  return (
    // The buttons sit under the counts: beside them, the counts break inside a size.
    <div className="flex flex-col gap-3" data-ui="RunsStorage">
      <div className="min-w-0">
        <p className="text-sm text-surface-100">{m.runs_storage_label()}</p>
        {store && (
          <>
            <p className="text-xs text-surface-300">
              {m.runs_storage_counts({ runs: store.runs, kept: store.kept, mods: store.mods })}
            </p>
            <p className="text-xs text-surface-400">
              {m.runs_storage_bytes({
                rows: diskSize(store.rows_bytes),
                tier: diskSize(store.tier_bytes),
                ingest: diskSize(store.ingest_bytes),
              })}
            </p>
            <p className="text-xs text-surface-400">
              {m.runs_storage_unused_bytes({ bytes: diskSize(store.ingest_unused_bytes) })}
            </p>
            <p className="text-xs text-surface-500">{m.runs_storage_tier_hint()}</p>
          </>
        )}
        {view.isError && <FailureNote failure={view.error} />}
        {failure && <FailureNote failure={failure} />}
      </div>
      <div className="flex flex-wrap gap-2">
        <Button
          size="sm"
          variant="outline"
          left={<BroomIcon weight="bold" />}
          disabled={!store || store.ingest_unused_bytes === 0}
          loading={clearUnused.isPending}
          onClick={() => setAsking("unused")}
        >
          {m.runs_storage_clear_unused_action()}
        </Button>
        <Button
          size="sm"
          variant="outline"
          left={<TrashIcon weight="bold" />}
          disabled={!store || store.runs === 0}
          loading={remove.isPending}
          onClick={() => setAsking("runs")}
        >
          {m.runs_storage_remove_action()}
        </Button>
      </div>
      <ConfirmRemove
        open={asking === "runs"}
        onClose={() => setAsking(null)}
        title={m.runs_storage_remove_title()}
        description={m.runs_storage_remove_description()}
        onConfirm={() => remove.mutate()}
      />
      <ConfirmRemove
        open={asking === "unused"}
        onClose={() => setAsking(null)}
        title={m.runs_storage_clear_unused_title()}
        description={m.runs_storage_clear_unused_description({
          bytes: diskSize(store?.ingest_unused_bytes ?? 0),
        })}
        onConfirm={() => clearUnused.mutate()}
      />
    </div>
  );
}
