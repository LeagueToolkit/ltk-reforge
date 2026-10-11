import { createFileRoute } from "@tanstack/react-router";

import { RecordDetail } from "@/modules/records";

function RecordRoute() {
  const { code } = Route.useParams();
  return <RecordDetail code={code} />;
}

export const Route = createFileRoute("/records/$code")({
  component: RecordRoute,
});
