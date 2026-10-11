import { Link } from "@tanstack/react-router";
import { useState } from "react";

import { FailureNote, Spinner } from "@/components";
import { m } from "@/i18n";

import { useRuns } from "../api/runs";
import { RunDetail } from "./RunDetail";
import { RunList } from "./RunList";

/**
 * Every run the app kept (#ADR-0091, #ADR-0096): the list on the left and the run picked on
 * the right, as LTK Manager's Mods page. The page renders what the store answers and adds no
 * rule about a mod (#ADR-0046).
 */
export function RunsPage() {
  const runs = useRuns();
  const [selected, setSelected] = useState<number | null>(null);
  const rows = runs.data ?? [];
  const chosen = rows.find((row) => row.id === selected) ?? null;
  // The ages are counted to when the list was last read.
  const now = new Date(runs.dataUpdatedAt || Date.now());

  return (
    <div className="flex flex-col gap-6" data-ui="RunsPage">
      <h1 className="font-display text-xl font-semibold text-surface-50">{m.runs_page_title()}</h1>
      {runs.isPending && <Spinner />}
      {runs.isError && <FailureNote failure={runs.error} />}
      {runs.isSuccess && rows.length === 0 && (
        <p className="text-sm text-surface-400 select-none" data-ui="RunsPage:empty">
          {m.runs_empty()}{" "}
          <Link to="/" className="text-accent-300 underline-offset-2 hover:underline">
            {m.runs_empty_link()}
          </Link>
        </p>
      )}
      {rows.length > 0 && (
        <div className="flex items-start gap-6">
          <RunList
            runs={rows}
            selected={selected}
            onSelect={setSelected}
            now={now}
            className="sticky top-0 max-h-[calc(100vh-4rem)] w-1/3 shrink-0 overflow-y-auto"
          />
          <div className="min-w-0 flex-1">
            {chosen ? (
              <RunDetail
                key={chosen.id}
                row={chosen}
                onRemoved={() => setSelected(null)}
                now={now}
              />
            ) : (
              <p className="text-sm text-surface-400 select-none">{m.runs_pick_hint()}</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
