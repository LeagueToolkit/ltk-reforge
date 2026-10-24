import { createFileRoute } from "@tanstack/react-router";

import { ManagerPage } from "@/modules/manager";

export const Route = createFileRoute("/manager")({
  component: ManagerPage,
  // The table of the library.
  staticData: { wide: true },
});
