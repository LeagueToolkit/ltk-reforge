import { createFileRoute } from "@tanstack/react-router";

import { ModPage } from "@/modules/mod";

export const Route = createFileRoute("/mod")({
  component: ModPage,
  // Tables of paths, which read better at the window's width.
  staticData: { wide: true },
});
