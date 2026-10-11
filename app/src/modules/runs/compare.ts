import type { RunRow } from "@/lib/bindings.gen";

/** The runs a run can be compared with: those of its mod, then the rest, each newest first. */
export interface Others {
  sameMod: RunRow[];
  rest: RunRow[];
}

/** Every kept run but `row`, those of its archive first, each group in the list's order. */
export function groupOthers(runs: RunRow[], row: RunRow): Others {
  const others = runs.filter((other) => other.id !== row.id);
  return {
    sameMod: others.filter((other) => other.sha256 === row.sha256),
    rest: others.filter((other) => other.sha256 !== row.sha256),
  };
}

/** The rows that differ first, each group in the order given. */
export function differingFirst<T extends { differs: boolean }>(rows: T[]): T[] {
  return [...rows.filter((row) => row.differs), ...rows.filter((row) => !row.differs)];
}

/** What a comparison's settings and inputs name, each with a message of its own. */
export const DIFF_KEYS = [
  "kind",
  "target",
  "anim_reuse",
  "keep_stale",
  "keep_dds",
  "game_events_only",
  "block",
  "checks_off",
  "fixups_off",
  "fixups_on",
  "rules",
  "program",
  "install_build",
  "history_head",
  "names",
  "schema_build",
] as const;

export type DiffKey = (typeof DIFF_KEYS)[number];

/** Whether a key the store sent is one the window has a message for. */
export function isDiffKey(key: string): key is DiffKey {
  return (DIFF_KEYS as readonly string[]).includes(key);
}

/** The keys whose values are switches, sent as `true` or `false`. */
export const SWITCH_KEYS: readonly DiffKey[] = ["keep_stale", "keep_dds", "game_events_only"];
