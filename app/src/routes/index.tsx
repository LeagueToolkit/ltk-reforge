import { createFileRoute } from "@tanstack/react-router";

import { RepairPage } from "@/modules/repair";

export const Route = createFileRoute("/")({
  component: RepairPage,
});
