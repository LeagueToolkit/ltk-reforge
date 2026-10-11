import { createFileRoute } from "@tanstack/react-router";

import { RunsPage } from "@/modules/runs";

export const Route = createFileRoute("/runs")({
  component: RunsPage,
  // Two panes side by side.
  staticData: { wide: true },
});
