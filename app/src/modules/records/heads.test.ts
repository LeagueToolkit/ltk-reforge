import { describe, expect, it } from "vitest";

import type { RecordHead } from "@/lib/bindings.gen";

import { matches, statusWord, titleParts } from "./heads";

function adr(status: string | null): RecordHead {
  return { code: "#ADR-0044", series: "adr", number: 44, title: "Forge is retired", status };
}

describe("statusWord", () => {
  it("says nothing of an accepted ADR, its later consequence superseded or not", () => {
    expect(statusWord(adr("Accepted (2026-10-04)"))).toBeNull();
    expect(statusWord(adr("Accepted (2026-10-04); the cap superseded by #ADR-0070"))).toBeNull();
  });

  it("names any other status by its first word", () => {
    expect(statusWord(adr("Proposed"))).toBe("Proposed");
    expect(statusWord(adr("Superseded by #ADR-0058"))).toBe("Superseded");
    expect(statusWord(adr("Withdrawn"))).toBe("Withdrawn");
  });

  it("says nothing of a record with no status", () => {
    expect(statusWord(adr(null))).toBeNull();
  });
});

describe("matches", () => {
  it("finds a record by its code or title, in any case", () => {
    expect(matches(adr(null), "adr-0044")).toBe(true);
    expect(matches(adr(null), "RETIRED")).toBe(true);
    expect(matches(adr(null), "census")).toBe(false);
    expect(matches(adr(null), "  ")).toBe(true);
  });
});

describe("titleParts", () => {
  it("splits a title at its code spans", () => {
    expect(titleParts("A `_base` suffix on the base mesh")).toEqual([
      { text: "A ", code: false },
      { text: "_base", code: true },
      { text: " suffix on the base mesh", code: false },
    ]);
    expect(titleParts("Static meshes as `.scb`")).toEqual([
      { text: "Static meshes as ", code: false },
      { text: ".scb", code: true },
    ]);
  });

  it("keeps a title with no span, and a lone backtick, as prose", () => {
    expect(titleParts("Champion reworks")).toEqual([{ text: "Champion reworks", code: false }]);
    expect(titleParts("A ` alone")).toEqual([{ text: "A ` alone", code: false }]);
  });
});
