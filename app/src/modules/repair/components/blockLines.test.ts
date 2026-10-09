import { describe, expect, it } from "vitest";

import { hasLine, severityLine, withLine } from "./blockLines";

describe("block lines", () => {
  it("a checkbox adds its line and takes it out again", () => {
    const on = withLine([], severityLine("error"), true);
    expect(on).toEqual(["severity=error"]);
    expect(withLine(on, severityLine("error"), false)).toEqual([]);
  });

  it("a line no checkbox expresses stays", () => {
    const lines = ["archive.no-skin", "record=#GAME-CHANGE-0005"];
    expect(withLine(lines, "archive", true)).toEqual([...lines, "archive"]);
    expect(withLine([...lines, "archive"], "archive", false)).toEqual(lines);
    expect(hasLine(lines, "archive")).toBe(false);
  });

  it("a line spelt with spaces or a wildcard counts as the term", () => {
    expect(hasLine([" severity = error "], "severity=error")).toBe(true);
    expect(hasLine(["check.*"], "check")).toBe(true);
    expect(withLine(["check.*", "check"], "check", false)).toEqual([]);
  });
});
