import { useEffect, useRef } from "react";

import { useSaveSettings, useSettings } from "@/modules/settings";

import { useFindGame } from "../api/inputs";

/**
 * Looks for the game once, when the app starts with none chosen, and chooses the one found.
 * A game the user chose is never replaced.
 */
export function GameFinder() {
  const settings = useSettings();
  const save = useSaveSettings();
  const find = useFindGame();
  const tried = useRef(false);

  useEffect(() => {
    if (tried.current || !settings.isSuccess || settings.data["inputs.game"]) {
      return;
    }
    tried.current = true;
    const current = settings.data;
    find.mutate(undefined, {
      onSuccess: (game) => {
        if (game) {
          save.mutate({ ...current, "inputs.game": game });
        }
      },
    });
  }, [settings.isSuccess, settings.data, find, save]);

  return null;
}
