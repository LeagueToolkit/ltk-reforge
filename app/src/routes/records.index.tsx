import { createFileRoute } from "@tanstack/react-router";

import { RecordsEmpty } from "@/modules/records";

export const Route = createFileRoute("/records/")({
  component: RecordsEmpty,
});
