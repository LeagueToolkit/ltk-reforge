import { createFileRoute } from "@tanstack/react-router";

import { RecordsPage } from "@/modules/records";

export const Route = createFileRoute("/records")({
  component: RecordsPage,
  // Two panes side by side.
  staticData: { wide: true },
});
