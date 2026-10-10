import { describe, expect, it } from "vitest";

import { AREA_GROUPS, AREAS, hasLine, severityLine, withLine } from "./blockLines";

describe("area groups", () => {
  it("offer each area once, under one stage, the run's inputs last", () => {
    expect(new Set(AREAS).size).toBe(AREAS.length);
    expect(AREAS).toHaveLength(25);
    expect(AREA_GROUPS.at(-1)?.stage).toBe("inputs");
    expect(AREA_GROUPS.find((group) => group.stage === "read")?.areas).toContain("archive");
  });
});

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
