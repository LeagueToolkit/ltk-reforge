import { describe, expect, it } from "vitest";

import type { BatchRow, OriginView } from "@/lib/bindings.gen";

import { movedToPremium, summaryText } from "./summary";

const counts = { fatal: 0, error: 0, warning: 0, note: 0 };
const none: OriginView = { verdict: "unknown", slot: null, strength: null, evidence: [] };

function row(fields: Partial<BatchRow>): BatchRow {
  return {
    index: 0,
    archive: "a.fantome",
    name: null,
    author: null,
    version: null,
    units: [],
    outcome: { outcome: "written" },
    saved: null,
    package: null,
    counts,
    millis: 0,
    ...fields,
  };
}

describe("summaryText", () => {
  it("names each mod with its version and author, and where each unit went", () => {
    const rows = [
      row({
        name: "Mimikyu Vex",
        version: "1.0.0",
        author: "someone",
        units: [
          { character: "Vex", source: 0, chosen: 0, origin: none },
          { character: "Janna", source: 0, chosen: 7, origin: none },
        ],
      }),
      row({ name: "Plain", units: [{ character: "Ahri", source: 3, chosen: null, origin: none }] }),
      row({ archive: "b.fantome", outcome: { outcome: "refused" } }),
      row({ name: "Bad", outcome: { outcome: "fault", detail: "x" } }),
    ];
    expect(summaryText(rows)).toBe(
      [
        "Mimikyu Vex 1.0.0 (someone): Vex base→base, Janna base→skin7",
        "Plain: Ahri skin3→game",
        "b.fantome: refused",
        "Bad: failed",
      ].join("\n"),
    );
  });

  it("says a repair was written, which has no units", () => {
    expect(summaryText([row({ name: "R" })])).toBe("R: written");
  });
});

describe("movedToPremium", () => {
  it("keeps the written rows with a base unit placed at a premium slot", () => {
    const moved = row({
      name: "Moved",
      units: [{ character: "Gwen", source: 0, chosen: 20, origin: none }],
    });
    const rows = [
      moved,
      row({ name: "Base", units: [{ character: "Vex", source: 0, chosen: 0, origin: none }] }),
      row({
        name: "Premium",
        units: [{ character: "Yone", source: 58, chosen: 58, origin: none }],
      }),
      row({ name: "Left", units: [{ character: "Ahri", source: 0, chosen: null, origin: none }] }),
      row({
        name: "Refused",
        outcome: { outcome: "refused" },
        units: [{ character: "Gwen", source: 0, chosen: 20, origin: none }],
      }),
    ];
    expect(movedToPremium(rows)).toEqual([moved]);
  });
});
