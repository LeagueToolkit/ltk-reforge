import type { ManagerMod } from "@/lib/bindings.gen";

/**
 * Whether a mod is one the batch ports from the manager's library as it stands: a plain
 * Fantome of a champion mod, with its archive (#ADR-0106).
 */
export function isLegacyChampionMod(mod: ManagerMod): boolean {
  const champion =
    mod.tags.includes("champion-skin") || (mod.champions.length > 0 && mod.maps.length === 0);
  return mod.format === "fantome" && !mod.extended && mod.archive !== null && champion;
}

/** Why a mod is not ticked by default. */
export type ManagerNote = "no-archive" | "modpkg" | "extended" | "not-champion";

/** Why a mod is not ticked by default, or nothing for one that is. */
export function noteOf(mod: ManagerMod): ManagerNote | null {
  if (mod.archive === null) {
    return "no-archive";
  }
  if (mod.format === "modpkg") {
    return "modpkg";
  }
  if (mod.extended) {
    return "extended";
  }
  return isLegacyChampionMod(mod) ? null : "not-champion";
}
