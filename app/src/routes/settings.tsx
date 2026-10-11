import { createFileRoute } from "@tanstack/react-router";

import { RunsStorage } from "@/modules/runs";
import { SettingsPage } from "@/modules/settings";

// The runs module reads the settings through the run page, so the route seats its row.
function Settings() {
  return <SettingsPage storage={<RunsStorage />} />;
}

export const Route = createFileRoute("/settings")({
  component: Settings,
});
