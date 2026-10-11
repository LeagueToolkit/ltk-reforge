import { PlayIcon, StopIcon } from "@phosphor-icons/react";
import { useCallback } from "react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import type { BatchRequest } from "@/lib/bindings.gen";
import { StaleInputsLink, useInputs, useInputsGate } from "@/modules/inputs";
import {
  runSettings,
  STANDARD,
  useArchiveDrop,
  useJobRunning,
  useRepairStore,
} from "@/modules/repair";
import { useSettings } from "@/modules/settings";

import { useBatchEvents, useBatchRunning, useRunBatch, useStopBatch } from "../api/batch";
import { summaryOf } from "../outcomes";
import { useBatchStore } from "../state/batchStore";
import { BatchSettings } from "./BatchSettings";
import { BatchTable } from "./BatchTable";
import { ModsCard } from "./ModsCard";

/** How far the batch under way is: the archive running, its place, and a bar. */
function BatchProgressLine() {
  const at = useBatchStore((state) => state.at);
  const done = useBatchStore((state) => state.rows.length);
  const stopping = useBatchStore((state) => state.stopping);
  if (!at) {
    return null;
  }
  const percent = at.total > 0 ? (done / at.total) * 100 : 0;
  return (
    <div className="flex flex-col gap-2" data-ui="BatchProgress">
      <p className="truncate text-sm text-surface-200 select-none">
        {m.batch_progress_label({ number: at.index + 1, total: at.total, archive: at.archive })}
      </p>
      <div className="h-1.5 overflow-hidden rounded-full bg-surface-800">
        <div className="h-full bg-accent-500 transition-[width]" style={{ width: `${percent}%` }} />
      </div>
      {stopping && (
        <p className="text-xs text-warning-text select-none">{m.batch_stopping_label()}</p>
      )}
    </div>
  );
}

/** The line under a batch that ended: how many rows ended each way, and its time. */
function BatchSummary() {
  const ended = useBatchStore((state) => state.ended);
  if (!ended) {
    return null;
  }
  const summary = summaryOf(ended.rows);
  const parts = [m.batch_summary_label(summary)];
  if (summary.stopped > 0) {
    parts.push(m.batch_summary_stopped_label({ count: summary.stopped }));
  }
  return (
    <p className="text-sm text-surface-200 select-none" data-ui="BatchSummary">
      {parts.join(", ")}{" "}
      <span className="text-surface-400">
        {m.batch_summary_time_label({ seconds: (ended.millis / 1000).toFixed(1) })}
      </span>
      {ended.stopped && <span className="text-surface-400"> {m.batch_stopped_label()}</span>}
    </p>
  );
}

/**
 * A folder of mods, or files dropped, run as one job with one set of settings (#ADR-0091): a
 * table of outcomes, each row its own run, kept in the store when it wrote its package. The
 * page adds no rule about a mod (#ADR-0046).
 */
export function BatchPage() {
  useBatchEvents();
  const archives = useBatchStore((state) => state.archives);
  const addArchives = useBatchStore((state) => state.addArchives);
  const kind = useBatchStore((state) => state.kind);
  const runPageSettings = useBatchStore((state) => state.runPageSettings);
  const rows = useBatchStore((state) => state.rows);
  const stopping = useBatchStore((state) => state.stopping);
  const advanced = useRepairStore((state) => state.advanced);
  const chosenOut = useSettings().data?.["output.folder"];
  const inputs = useInputs();
  const running = useBatchRunning();
  const busy = useJobRunning();
  const runBatch = useRunBatch();
  const stop = useStopBatch();
  const { gate, dialog } = useInputsGate();

  const onDrop = useCallback(
    (dropped: string[]) => {
      if (!busy) {
        addArchives(dropped);
      }
    },
    [busy, addArchives],
  );
  const over = useArchiveDrop(onDrop);

  const start = () => {
    const request: BatchRequest = {
      kind,
      archives,
      settings: runSettings(runPageSettings ? advanced : STANDARD),
      out: chosenOut ?? null,
    };
    gate(() => runBatch.mutate(request));
  };

  return (
    <div className="flex flex-col gap-6" data-ui="BatchPage">
      <div className="flex items-center gap-3">
        <h1 className="font-display text-xl font-semibold text-surface-50">
          {m.batch_page_title()}
        </h1>
        <StaleInputsLink />
      </div>
      <p className="text-sm text-surface-400 select-none">{m.batch_page_description()}</p>
      <ModsCard busy={busy} over={over} />
      <BatchSettings busy={busy} />
      <div className="flex items-center gap-3">
        {running ? (
          <Button
            variant="outline"
            left={<StopIcon weight="bold" />}
            loading={stopping}
            onClick={() => stop.mutate()}
          >
            {m.batch_stop_action()}
          </Button>
        ) : (
          <Button
            variant="filled"
            left={<PlayIcon weight="bold" />}
            disabled={archives.length === 0 || busy || inputs.isPending}
            onClick={start}
          >
            {m.batch_start_action()}
          </Button>
        )}
      </div>
      {dialog}
      {stop.isError && <FailureNote failure={stop.error} />}
      {runBatch.isError && <FailureNote failure={runBatch.error} />}
      {running && <BatchProgressLine />}
      <BatchSummary />
      {rows.length > 0 && <BatchTable rows={rows} />}
    </div>
  );
}
