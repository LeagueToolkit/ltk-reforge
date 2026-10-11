/** A line of a unified diff: in both texts, only in the later, or only in the earlier. */
export interface DiffLine {
  kind: "same" | "added" | "removed";
  text: string;
}

/** The most lines either text has for the two to be compared. */
export const MAX_DIFF_LINES = 20_000;

/**
 * The most cells the table of the lines between the common head and tail holds: 32 MB of
 * 16-bit counts, which a line count under `MAX_DIFF_LINES` always fits.
 */
const MAX_TABLE_CELLS = 16 * 1024 * 1024;

function lines(text: string): string[] {
  const split = text.split("\n");
  // A text that ends in a line break has no line after it.
  if (split.length > 0 && split[split.length - 1] === "") {
    split.pop();
  }
  return split;
}

/**
 * The lines of `before` and `after` as a unified diff, by their longest common subsequence.
 * The common head and tail are matched first, so two versions of an object that differ in a
 * few lines compare at once. Null when either text has more than `MAX_DIFF_LINES` lines, or
 * the lines between the common head and tail are too many to compare.
 */
export function lineDiff(before: string, after: string): DiffLine[] | null {
  const a = lines(before);
  const b = lines(after);
  if (a.length > MAX_DIFF_LINES || b.length > MAX_DIFF_LINES) {
    return null;
  }
  let head = 0;
  while (head < a.length && head < b.length && a[head] === b[head]) {
    head += 1;
  }
  let tail = 0;
  while (
    tail < a.length - head &&
    tail < b.length - head &&
    a[a.length - 1 - tail] === b[b.length - 1 - tail]
  ) {
    tail += 1;
  }
  const midA = a.slice(head, a.length - tail);
  const midB = b.slice(head, b.length - tail);
  const n = midA.length;
  const m = midB.length;
  if ((n + 1) * (m + 1) > MAX_TABLE_CELLS) {
    return null;
  }

  // `common[i * (m + 1) + j]`: the longest common subsequence of `midA[i..]` and `midB[j..]`.
  const width = m + 1;
  const common = new Uint16Array((n + 1) * width);
  for (let i = n - 1; i >= 0; i -= 1) {
    for (let j = m - 1; j >= 0; j -= 1) {
      common[i * width + j] =
        midA[i] === midB[j]
          ? common[(i + 1) * width + j + 1] + 1
          : Math.max(common[(i + 1) * width + j], common[i * width + j + 1]);
    }
  }

  const out: DiffLine[] = a.slice(0, head).map((text) => ({ kind: "same", text }));
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (midA[i] === midB[j]) {
      out.push({ kind: "same", text: midA[i] });
      i += 1;
      j += 1;
    } else if (common[(i + 1) * width + j] >= common[i * width + j + 1]) {
      out.push({ kind: "removed", text: midA[i] });
      i += 1;
    } else {
      out.push({ kind: "added", text: midB[j] });
      j += 1;
    }
  }
  for (; i < n; i += 1) {
    out.push({ kind: "removed", text: midA[i] });
  }
  for (; j < m; j += 1) {
    out.push({ kind: "added", text: midB[j] });
  }
  for (const text of a.slice(a.length - tail)) {
    out.push({ kind: "same", text });
  }
  return out;
}
