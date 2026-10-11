import { Link } from "@tanstack/react-router";
import { useState } from "react";

import { FailureNote, Spinner } from "@/components";
import { m } from "@/i18n";

import { type Opened, useOpenDrop } from "../api/drop";
import { useOpenRun, useRuns } from "../api/runs";
import { OpenMenu } from "./OpenMenu";
import { RunDetail } from "./RunDetail";
import { RunList } from "./RunList";

/**
 * Every run the app kept (#ADR-0091, #ADR-0096): the list on the left and the run picked on
 * the right, as LTK Manager's Mods page. A project file or a package, picked or dropped on the
 * page, opens as a kept run. The page renders what the store answers and adds no rule about a
 * mod (#ADR-0046).
 */
export function RunsPage() {
  const runs = useRuns();
  const openRun = useOpenRun();
  const [selected, setSelected] = useState<number | null>(null);
  const rows = runs.data ?? [];
  const chosen = rows.find((row) => row.id === selected) ?? null;
  // The ages are counted to when the list was last read.
  const now = new Date(runs.dataUpdatedAt || Date.now());
  const open = (opened: Opened) => openRun.mutate(opened, { onSuccess: (id) => setSelected(id) });
  const over = useOpenDrop(open);
  const menu = <OpenMenu onOpen={open} pending={openRun.isPending} />;
  const refusal = openRun.error && <FailureNote failure={openRun.error} />;

  return (
    <div className="flex flex-col gap-6" data-ui="RunsPage">
      <h1 className="font-display text-xl font-semibold text-surface-50">{m.runs_page_title()}</h1>
      {over && (
        <p
          className="rounded-lg border border-dashed border-accent-500/60 px-4 py-3 text-sm text-accent-200 select-none"
          data-ui="RunsPage:drop"
        >
          {m.runs_open_drop_hint()}
        </p>
      )}
      {runs.isPending && <Spinner />}
      {runs.isError && <FailureNote failure={runs.error} />}
      {runs.isSuccess && rows.length === 0 && (
        <div className="flex flex-col items-start gap-3" data-ui="RunsPage:empty">
          <p className="text-sm text-surface-400 select-none">
            {m.runs_empty()}{" "}
            <Link to="/" className="text-accent-300 underline-offset-2 hover:underline">
              {m.runs_empty_link()}
            </Link>
          </p>
          {menu}
          {refusal}
        </div>
      )}
      {rows.length > 0 && (
        <div className="flex items-start gap-6">
          <RunList
            runs={rows}
            selected={selected}
            onSelect={setSelected}
            now={now}
            actions={menu}
            note={refusal}
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
