import { createRootRoute, Outlet } from "@tanstack/react-router";

import { Shell } from "@/modules/shell";

export const Route = createRootRoute({
  component: () => (
    <Shell>
      <Outlet />
    </Shell>
  ),
});
