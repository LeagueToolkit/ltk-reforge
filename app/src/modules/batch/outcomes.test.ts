import { describe, expect, it } from "vitest";

import type { BatchRow, Outcome } from "@/lib/bindings.gen";

import { summaryOf, withArchives } from "./outcomes";

function row(index: number, outcome: Outcome): BatchRow {
  return {
    index,
    archive: `mod${index}.fantome`,
    name: null,
    author: null,
    version: null,
    units: [],
    outcome,
    saved: null,
    package: null,
    counts: { fatal: 0, error: 0, warning: 0, note: 0 },
    millis: 0,
  };
}

describe("summaryOf", () => {
  it("counts written, refused or halted, failed or unreadable, and stopped rows", () => {
    const rows = [
      row(0, { outcome: "written" }),
      row(1, { outcome: "written" }),
      row(2, { outcome: "refused" }),
      row(3, { outcome: "halted", code: "archive.no-skin" }),
      row(4, { outcome: "fault", detail: "the game did not read" }),
      row(5, { outcome: "unreadable" }),
      row(6, { outcome: "cancelled", stage: "fix" }),
    ];

    expect(summaryOf(rows)).toEqual({ written: 2, refused: 2, failed: 2, stopped: 1 });
  });

  it("counts nothing of no rows", () => {
    expect(summaryOf([])).toEqual({ written: 0, refused: 0, failed: 0, stopped: 0 });
  });
});

describe("withArchives", () => {
  it("adds the paths not listed, after the ones listed, each once", () => {
    expect(withArchives(["a.zip", "b.zip"], ["b.zip", "c.zip", "c.zip"])).toEqual([
      "a.zip",
      "b.zip",
      "c.zip",
    ]);
  });
});
