import { createFileRoute } from "@tanstack/react-router";

import { InputsPage } from "@/modules/inputs";

export const Route = createFileRoute("/inputs")({
  component: InputsPage,
});
