import { ArrowCounterClockwiseIcon, BroomIcon, TrashIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import type { StoreState } from "@/lib/bindings.gen";
import { useStoreState } from "@/lib/storeState";
import { ConfirmRemove, diskSize } from "@/modules/inputs";

import { useClearUnusedIngest, useRemoveAllRuns, useResetStore, useStoreView } from "../api/runs";

/** What the storage group asks about before it removes. */
type Asking = "runs" | "unused" | "reset" | null;

/**
 * The store of runs as the settings page's storage group shows it: its counts and its bytes
 * while it is open; what a reset loses while another build's rows keep it closed; why it did
 * not open otherwise. A reset is offered in every state, asked about first.
 */
export function RunsStorage() {
  const state = useStoreState();
  const open = state.data?.state === "open";
  const view = useStoreView(open);
  const remove = useRemoveAllRuns();
  const clearUnused = useClearUnusedIngest();
  const reset = useResetStore();
  const [asking, setAsking] = useState<Asking>(null);
  const store = open ? view.data : undefined;
  const failure = remove.error ?? clearUnused.error ?? reset.error;
  const askReset = () => setAsking("reset");

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
        {state.isError && <FailureNote failure={state.error} />}
        {open && view.isError && <FailureNote failure={view.error} />}
        {failure && <FailureNote failure={failure} />}
      </div>
      {state.data && state.data.state !== "open" && (
        <ClosedStore state={state.data} resetting={reset.isPending} onReset={askReset} />
      )}
      {open && (
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
          <Button
            size="sm"
            variant="ghost"
            className="ml-auto"
            left={<ArrowCounterClockwiseIcon weight="bold" />}
            loading={reset.isPending}
            onClick={askReset}
          >
            {m.runs_storage_reset_action()}
          </Button>
        </div>
      )}
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
      <ConfirmRemove
        open={asking === "reset"}
        onClose={() => setAsking(null)}
        title={m.runs_storage_reset_title()}
        description={
          open ? m.runs_storage_reset_open_description() : m.runs_storage_reset_description()
        }
        onConfirm={() => reset.mutate()}
      />
    </div>
  );
}

interface ClosedStoreProps {
  state: Exclude<StoreState, { state: "open" }>;
  resetting: boolean;
  onReset: () => void;
}

/**
 * A store that is not open: what a reset of another build's rows loses, or why it failed,
 * with the reset that opens it anew.
 */
function ClosedStore({ state, resetting, onReset }: ClosedStoreProps) {
  const outdated = state.state === "outdated";
  return (
    <div
      className="flex flex-col items-start gap-3 rounded-md border border-warning/40 bg-warning/10 p-3"
      data-ui="RunsStorage:closed"
    >
      {state.state === "outdated" ? (
        <p className="text-xs text-surface-200 select-none">
          {m.runs_storage_outdated({
            runs: state.runs,
            kept: state.kept,
            bytes: diskSize(state.rows_bytes + state.tier_bytes),
          })}
        </p>
      ) : (
        <div className="min-w-0">
          <p className="text-xs text-surface-200 select-none">{m.runs_storage_failed()}</p>
          <p className="font-mono text-xs break-words text-surface-400">{state.detail}</p>
        </div>
      )}
      <Button
        size="sm"
        variant="danger"
        left={<ArrowCounterClockwiseIcon weight="bold" />}
        loading={resetting}
        onClick={onReset}
      >
        {outdated ? m.runs_storage_reset_action() : m.runs_storage_retry_action()}
      </Button>
    </div>
  );
}
