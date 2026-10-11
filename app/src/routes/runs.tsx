import { createFileRoute } from "@tanstack/react-router";

import { RunsPage } from "@/modules/runs";

/** The run selected, by its number in the store, when a link names one. */
interface RunsSearch {
  run?: number;
}

export const Route = createFileRoute("/runs")({
  component: RunsPage,
  // The router reads `?run=12` as the number 12.
  validateSearch: (search: Record<string, unknown>): RunsSearch =>
    typeof search.run === "number" && Number.isSafeInteger(search.run) ? { run: search.run } : {},
  // Two panes side by side.
  staticData: { wide: true },
});
