import { useEffect, useSyncExternalStore } from "react";

import type { Theme } from "@/lib/bindings.gen";

import { useSettings } from "./api/settings";

/** A theme the stylesheet draws: `global.css` holds dark on `:root` and light under `data-theme`. */
export type DrawnTheme = "dark" | "light";

/** The theme drawn for a setting, given whether the system prefers dark. */
export function drawnTheme(setting: Theme, systemPrefersDark: boolean): DrawnTheme {
  if (setting === "system") {
    return systemPrefersDark ? "dark" : "light";
  }
  return setting;
}

const DARK = "(prefers-color-scheme: dark)";

function subscribe(onChange: () => void) {
  const query = window.matchMedia(DARK);
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
}

/** Puts the theme setting on the document, and follows the system's while it is `system`. */
export function useTheme() {
  const { data: settings } = useSettings();
  const systemPrefersDark = useSyncExternalStore(subscribe, () => window.matchMedia(DARK).matches);
  const theme = drawnTheme(settings?.["appearance.theme"] ?? "system", systemPrefersDark);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.style.colorScheme = theme;
  }, [theme]);
}
