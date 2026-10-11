import { TrashIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import { ConfirmRemove, diskSize } from "@/modules/inputs";

import { useRemoveAllRuns, useStoreView } from "../api/runs";

/** The store of runs as the settings page's storage group shows it: its counts, its bytes. */
export function RunsStorage() {
  const view = useStoreView();
  const remove = useRemoveAllRuns();
  const [asking, setAsking] = useState(false);
  const store = view.data;

  return (
    <div className="flex items-start justify-between gap-4" data-ui="RunsStorage">
      <div className="min-w-0 flex-1">
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
            <p className="text-xs text-surface-500">{m.runs_storage_tier_hint()}</p>
          </>
        )}
        {view.isError && <FailureNote failure={view.error} />}
        {remove.isError && <FailureNote failure={remove.error} />}
      </div>
      <Button
        size="sm"
        variant="outline"
        left={<TrashIcon weight="bold" />}
        disabled={!store || store.runs === 0}
        loading={remove.isPending}
        onClick={() => setAsking(true)}
      >
        {m.runs_storage_remove_action()}
      </Button>
      <ConfirmRemove
        open={asking}
        onClose={() => setAsking(false)}
        title={m.runs_storage_remove_title()}
        description={m.runs_storage_remove_description()}
        onConfirm={() => remove.mutate()}
      />
    </div>
  );
}
