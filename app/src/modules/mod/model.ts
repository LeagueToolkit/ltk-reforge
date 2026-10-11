import type { ExploreView, Finding, KindCount, SourceCount, UnitView } from "@/lib/bindings.gen";
import { bySourceFact } from "@/modules/files";

/** A row of a count by kind or by provenance fact. */
export interface CountRow {
  /** The kind, or the fact. */
  key: string;
  files: number;
  bytes: number;
}

/** The files per kind, in the order the report gives them: the library's order of kinds. */
export function kindRows(kinds: KindCount[]): CountRow[] {
  return kinds.map(({ kind, files, bytes }) => ({ key: kind, files, bytes }));
}

/** The files per provenance fact, in the order the library declares the facts. */
export function sourceRows(sources: SourceCount[]): CountRow[] {
  return sources
    .map(({ fact, files, bytes }) => ({ key: fact, files, bytes }))
    .sort((a, b) => bySourceFact(a.key, b.key));
}

/** Whether a unit's skin entry names another slot than the one its bin sits in. */
export function entrySlotDiffers(unit: UnitView): boolean {
  return unit.entry_name_slot != null && unit.entry_name_slot !== unit.slot;
}

/** The finding that ended the explore's run, when one did. */
export function haltingFinding(view: ExploreView): Finding | null {
  if (view.halted == null) {
    return null;
  }
  return view.findings.find((finding) => finding.id === view.halted) ?? null;
}

/**
 * Whether the explore's run ended before it knew anything of the mod: halted or faulted, with
 * no dating, character, unit, WAD or count. An overview of empty cards would read as an empty
 * archive.
 */
export function endedEmpty(view: ExploreView): boolean {
  const ended = view.halted != null || view.fault != null;
  return (
    ended &&
    view.dating == null &&
    [view.characters, view.units, view.wads, view.kinds, view.sources].every(
      (list) => list.length === 0,
    )
  );
}

/** Whether a word from the backend is one of the keys the app has messages for. */
export function isOneOf<K extends string>(keys: readonly K[], word: string): word is K {
  return (keys as readonly string[]).includes(word);
}
