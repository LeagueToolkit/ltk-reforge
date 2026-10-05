import { createRootRoute, Outlet } from "@tanstack/react-router";

import { GameFinder, StaleInputsMark } from "@/modules/inputs";
import { Shell } from "@/modules/shell";

export const Route = createRootRoute({
  component: () => (
    <Shell inputsMark={<StaleInputsMark />}>
      <GameFinder />
      <Outlet />
    </Shell>
  ),
});
