import { createFileRoute } from "@tanstack/react-router";

import { HistoryPage } from "@/modules/history";

export const Route = createFileRoute("/history")({
  component: HistoryPage,
  // The tables of WADs, objects and entries, and an object's text.
  staticData: { wide: true },
});
