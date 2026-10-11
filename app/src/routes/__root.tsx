import { createRootRoute, Outlet, useMatches } from "@tanstack/react-router";

import { GameFinder, StaleInputsMark } from "@/modules/inputs";
import { Shell } from "@/modules/shell";

declare module "@tanstack/react-router" {
  interface StaticDataRouteOption {
    /** The page takes the window's whole width (`Shell`'s `wide`). */
    wide?: boolean;
  }
}

function Frame() {
  const wide = useMatches({ select: (matches) => matches.some((m) => m.staticData.wide) });
  return (
    <Shell inputsMark={<StaleInputsMark />} wide={wide}>
      <GameFinder />
      <Outlet />
    </Shell>
  );
}

export const Route = createRootRoute({
  component: Frame,
});
