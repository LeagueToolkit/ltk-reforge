import { describe, expect, it } from "vitest";

import type { BatchRow, BatchUnit, OriginView } from "@/lib/bindings.gen";

import { hasPaste, isPaste, movesToPremium, originText, reportText } from "./scan";

function unit(origin: Partial<OriginView>, chosen: number | null = 0): BatchUnit {
  return {
    character: "Vex",
    source: 0,
    chosen,
    origin: { verdict: "unknown", slot: null, strength: null, evidence: [], ...origin },
  };
}

function row(units: BatchUnit[]): BatchRow {
  return {
    index: 0,
    archive: "a.fantome",
    name: null,
    author: null,
    version: null,
    units,
    outcome: { outcome: "scanned" },
    saved: null,
    package: null,
    counts: { fatal: 0, error: 0, warning: 0, note: 0 },
    millis: 0,
  };
}

describe("isPaste", () => {
  it("takes a premium origin on strong mesh or skeleton evidence alone", () => {
    const paste = unit({ verdict: "premium", slot: 20, strength: "strong", evidence: ["mesh"] });
    expect(isPaste(paste)).toBe(true);
    expect(
      isPaste(unit({ verdict: "premium", slot: 20, strength: "strong", evidence: ["skin-name"] })),
    ).toBe(false);
    expect(isPaste(unit({ verdict: "premium", slot: 20, strength: "weak", evidence: [] }))).toBe(
      false,
    );
    expect(isPaste(unit({ verdict: "base", strength: "strong", evidence: ["mesh"] }))).toBe(false);
    expect(hasPaste(row([unit({ verdict: "base" }), paste]))).toBe(true);
  });
});

describe("originText", () => {
  it("spells each verdict with what it rests on", () => {
    expect(
      originText(
        unit({ verdict: "premium", slot: 20, strength: "strong", evidence: ["mesh", "skeleton"] }),
      ),
    ).toBe("paste of skin 20 (mesh, skeleton)");
    expect(
      originText(
        unit({ verdict: "premium", slot: 7, strength: "strong", evidence: ["skin-name"] }),
      ),
    ).toBe("from skin 7 (skin name)");
    expect(originText(unit({ verdict: "base", strength: "strong", evidence: ["resolver"] }))).toBe(
      "base (resolver)",
    );
    expect(originText(unit({ verdict: "custom" }))).toBe("own work");
    expect(originText(unit({}))).toBe("unknown");
  });
});

describe("movesToPremium", () => {
  it("sees a base unit that would land at a premium slot", () => {
    expect(movesToPremium(row([unit({}, 20)]))).toBe(true);
    expect(movesToPremium(row([unit({}, 0)]))).toBe(false);
    expect(movesToPremium(row([unit({}, null)]))).toBe(false);
  });
});

describe("reportText", () => {
  it("lists the mods that need a premium skin, with each unit's origin", () => {
    const moved = {
      ...row([unit({ verdict: "premium", slot: 20, strength: "strong", evidence: ["mesh"] }, 20)]),
      name: "A mod",
      version: "1.0.0",
      author: "someone",
    };
    const kept = row([unit({ verdict: "base", strength: "strong", evidence: [] }, 0)]);
    expect(reportText([kept, moved])).toBe(
      "A mod 1.0.0 (someone): Vex: paste of skin 20 (mesh) → skin 20",
    );
    expect(reportText([kept])).toBe("");
  });
});
