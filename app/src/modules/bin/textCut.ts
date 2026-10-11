/**
 * A name cut to the box it is drawn in.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/shared/utils/textCut.ts`. Cut:
 * `splitPath`, `pathUnder` and `nameColumn`, which the link chips and the cards use.
 */

const ELLIPSIS = "…";

/**
 * `text` cut in its middle to `fit` characters, an ellipsis standing in for what was cut.
 *
 * The middle, because a name's two ends are what tell it from its neighbours. A `null` fit is a
 * box not yet measured.
 */
export function cutText(text: string, fit: number | null): string {
  if (fit === null || text.length <= fit) return text;
  if (fit <= 0) return "";
  const kept = fit - 1;
  const head = Math.ceil(kept / 2);
  const tail = kept - head;
  return text.slice(0, head) + ELLIPSIS + (tail > 0 ? text.slice(-tail) : "");
}

/** The characters `box` pixels hold at `ch` pixels each, or null before either is measured. */
export function charsIn(box: number, ch: number): number | null {
  if (box <= 0 || ch <= 0) return null;
  return Math.floor(box / ch);
}
