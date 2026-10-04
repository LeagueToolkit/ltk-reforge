import { createRootRoute, Outlet } from "@tanstack/react-router";

import { GameFinder } from "@/modules/inputs";
import { Shell } from "@/modules/shell";

export const Route = createRootRoute({
  component: () => (
    <Shell>
      <GameFinder />
      <Outlet />
    </Shell>
  ),
});
