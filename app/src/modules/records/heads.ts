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

/** A run of a record's title: prose, or the text of a code span. */
export interface TitlePart {
  text: string;
  code: boolean;
}

/**
 * A record's title split at its code spans, a heading's Markdown (`` Static meshes as `.scb` ``).
 * A backtick with no partner is prose.
 */
export function titleParts(title: string): TitlePart[] {
  const parts: TitlePart[] = [];
  const spans = /`([^`]+)`/g;
  let at = 0;
  for (let span = spans.exec(title); span; span = spans.exec(title)) {
    if (span.index > at) {
      parts.push({ text: title.slice(at, span.index), code: false });
    }
    parts.push({ text: span[1], code: true });
    at = span.index + span[0].length;
  }
  if (at < title.length) {
    parts.push({ text: title.slice(at), code: false });
  }
  return parts;
}

/** Whether a record's code or title holds the words typed, in any case. */
export function matches(head: RecordHead, filter: string): boolean {
  const words = filter.trim().toLowerCase();
  return !words || [head.code, head.title].some((text) => text.toLowerCase().includes(words));
}
