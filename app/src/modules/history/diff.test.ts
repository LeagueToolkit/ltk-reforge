import { describe, expect, it } from "vitest";

import { lineDiff, MAX_DIFF_LINES } from "./diff";

const same = (text: string) => ({ kind: "same", text });
const added = (text: string) => ({ kind: "added", text });
const removed = (text: string) => ({ kind: "removed", text });

describe("lineDiff", () => {
  it("marks every line of two equal texts the same", () => {
    expect(lineDiff("a\nb\n", "a\nb\n")).toEqual([same("a"), same("b")]);
  });

  it("marks a line only the later text has as added", () => {
    expect(lineDiff("a\nc\n", "a\nb\nc\n")).toEqual([same("a"), added("b"), same("c")]);
  });

  it("marks a line only the earlier text has as removed", () => {
    expect(lineDiff("a\nb\nc\n", "a\nc\n")).toEqual([same("a"), removed("b"), same("c")]);
  });

  it("marks a changed line as the earlier removed and the later added", () => {
    expect(lineDiff("a\nb\nc\n", "a\nB\nc\n")).toEqual([
      same("a"),
      removed("b"),
      added("B"),
      same("c"),
    ]);
  });

  it("matches lines between changes by their longest common run", () => {
    expect(lineDiff("x\na\nb\ny\n", "a\nz\nb\n")).toEqual([
      removed("x"),
      same("a"),
      added("z"),
      same("b"),
      removed("y"),
    ]);
  });

  it("compares no text of more lines than the bound", () => {
    const long = "line\n".repeat(MAX_DIFF_LINES + 1);
    expect(lineDiff(long, "line\n")).toBeNull();
    expect(lineDiff("line\n", long)).toBeNull();
    const atBound = "line\n".repeat(MAX_DIFF_LINES);
    expect(lineDiff(atBound, atBound)).toHaveLength(MAX_DIFF_LINES);
  });
});
