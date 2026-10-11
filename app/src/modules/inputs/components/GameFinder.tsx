import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";

import { useSettings } from "@/modules/settings";

import { installsQuery, useChooseInstall } from "../api/installs";

/**
 * Looks for the installs once, when the app starts with none chosen, and chooses the live one,
 * else the first found. An install the user chose is never replaced.
 */
export function GameFinder() {
  const settings = useSettings();
  const queryClient = useQueryClient();
  const choose = useChooseInstall();
  const tried = useRef(false);

  useEffect(() => {
    if (tried.current || !settings.isSuccess || settings.data["inputs.game"]) {
      return;
    }
    tried.current = true;
    void queryClient.fetchQuery(installsQuery()).then(
      (installs) => {
        const found = installs.find((install) => install.source === "live") ?? installs[0];
        if (found) {
          choose.mutate(found.path);
        }
      },
      // A detection that fails chooses nothing; the inputs page lists the installs again.
      () => undefined,
    );
  }, [settings.isSuccess, settings.data, queryClient, choose]);

  return null;
}
