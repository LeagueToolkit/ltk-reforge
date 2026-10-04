import type { ChangeView, FixupChanges, ObjectView, OutcomeView } from "@/lib/bindings.gen";

/** The outcomes in the order the library raises their findings. */
export const OUTCOMES: readonly OutcomeView[] = ["changed", "removed", "lost", "unconvertible"];

/** The code of the finding a fixup raises for its changes of one outcome. */
export function codeOf(outcome: OutcomeView): string {
  return `fixup.${outcome}`;
}

/** The outcome a `fixup.*` finding counts, by its code. None for another finding. */
export function outcomeOfCode(code: string): OutcomeView | null {
  return OUTCOMES.find((outcome) => codeOf(outcome) === code) ?? null;
}

/** A change with the fixup that made it and the object it is in. */
export interface ChangeItem {
  fixup: string;
  change: ChangeView;
  object: ObjectView | undefined;
}

/** Every change of every fixup, in the order made, each with its object. */
export function itemsOf(changes: FixupChanges[], objects: ObjectView[]): ChangeItem[] {
  const byId = new Map(objects.map((object) => [object.id, object]));
  return changes.flatMap((fixup) =>
    fixup.changes.map((change) => ({
      fixup: fixup.fixup,
      change,
      object: byId.get(change.object),
    })),
  );
}

function textOf(item: ChangeItem): string {
  const { change, object } = item;
  const after = change.after;
  const parts = [
    item.fixup,
    change.field,
    change.path,
    change.entry,
    change.before?.text ?? "",
    after.as === "value" ? after.text : "",
    after.as === "moved" ? after.to : "",
    object?.entry ?? "",
    object?.class ?? "",
    object?.bin ?? "",
  ];
  return parts.join("\n").toLowerCase();
}

/** Whether a change holds every word of a filter, in any of its texts. */
export function matchesText(item: ChangeItem, filter: string): boolean {
  const words = filter.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) {
    return true;
  }
  const text = textOf(item);
  return words.every((word) => text.includes(word));
}

/** Items grouped by a key, each group in the order of its first item. */
export function groupBy<T>(items: T[], key: (item: T) => string): { key: string; items: T[] }[] {
  const groups = new Map<string, T[]>();
  for (const item of items) {
    const at = key(item);
    const group = groups.get(at);
    if (group) {
      group.push(item);
    } else {
      groups.set(at, [item]);
    }
  }
  return [...groups].map(([at, grouped]) => ({ key: at, items: grouped }));
}

/** How many changes there are of each outcome. */
export function countOutcomes(items: ChangeItem[]): Record<OutcomeView, number> {
  const counts: Record<OutcomeView, number> = { changed: 0, removed: 0, lost: 0, unconvertible: 0 };
  for (const item of items) {
    counts[item.change.outcome] += 1;
  }
  return counts;
}

/** The fields of some changes and how many changes each has, the most first. */
export function fieldCounts(items: ChangeItem[]): { field: string; count: number }[] {
  return groupBy(items, (item) => item.change.field)
    .filter((group) => group.key !== "")
    .map((group) => ({ field: group.key, count: group.items.length }))
    .sort((a, b) => b.count - a.count);
}

/** One segment of a path, and whether the path it came from lacks it. */
export interface PathPart {
  text: string;
  added: boolean;
}

/**
 * A path a value moved to, split into its segments, each marked when it is not in the longest
 * run of segments the two paths share in order: a struct a move added, a field renamed.
 */
export function movedParts(from: string, to: string): PathPart[] {
  const old = from.split(".");
  const now = to.split(".");
  // longest[i][j]: the most segments old[i..] and now[j..] share in order.
  const longest = Array.from({ length: old.length + 1 }, () => Array(now.length + 1).fill(0));
  for (let i = old.length - 1; i >= 0; i -= 1) {
    for (let j = now.length - 1; j >= 0; j -= 1) {
      longest[i][j] =
        old[i] === now[j]
          ? longest[i + 1][j + 1] + 1
          : Math.max(longest[i + 1][j], longest[i][j + 1]);
    }
  }
  const parts: PathPart[] = [];
  let i = 0;
  let j = 0;
  while (j < now.length) {
    if (i < old.length && old[i] === now[j]) {
      parts.push({ text: now[j], added: false });
      i += 1;
      j += 1;
    } else if (i < old.length && longest[i + 1][j] >= longest[i][j + 1]) {
      i += 1;
    } else {
      parts.push({ text: now[j], added: true });
      j += 1;
    }
  }
  return parts;
}

/** An entry name split at its last `/`: the folder, and the name. */
export function splitEntry(entry: string): { folder: string; name: string } {
  const cut = entry.lastIndexOf("/");
  return cut < 0
    ? { folder: "", name: entry }
    : { folder: entry.slice(0, cut + 1), name: entry.slice(cut + 1) };
}
