import { createFileRoute } from "@tanstack/react-router";

import { BatchPage } from "@/modules/batch";

export const Route = createFileRoute("/batch")({
  component: BatchPage,
  // The table of outcomes.
  staticData: { wide: true },
});
