import type { RecordHead, Series } from "@/lib/bindings.gen";

/** The series in the order the list shows them. */
export const SERIES: Series[] = ["adr", "game-change", "game-behavior", "mod-artifact"];

/**
 * An ADR's status in a word when it is not accepted: `Superseded` of `Superseded by
 * #ADR-0044`. Null for an accepted ADR and for the other series, which carry none.
 */
export function statusWord(head: RecordHead): string | null {
  const status = head.status?.trim();
  if (!status || /^Accepted\b/.test(status)) {
    return null;
  }
  return status.split(/[\s(;]/)[0] || null;
}

/** Whether a record's code or title holds the words typed, in any case. */
export function matches(head: RecordHead, filter: string): boolean {
  const words = filter.trim().toLowerCase();
  return !words || [head.code, head.title].some((text) => text.toLowerCase().includes(words));
}
