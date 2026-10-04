import { createFileRoute } from "@tanstack/react-router";

import { RepairPage } from "@/modules/fixups";

export const Route = createFileRoute("/")({
  component: RepairPage,
});
