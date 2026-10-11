import { describe, expect, it } from "vitest";

import type { ExploreView, Finding, UnitView } from "@/lib/bindings.gen";

import { endedEmpty, entrySlotDiffers, haltingFinding, kindRows, sourceRows } from "./model";

function unit(slot: number, entryNameSlot: number | null): UnitView {
  return {
    id: "%u0",
    character_name: "test",
    slot,
    file: "data/characters/test/skins/skin0.bin",
    entry_name_slot: entryNameSlot,
    loads: 1,
  };
}

function finding(id: number): Finding {
  return {
    id,
    code: "archive.unreadable",
    severity: "fatal",
    channel: "run",
    stage: "read",
    subject: { kind: "run", id: "", name: "" },
    message: "the archive does not read",
    data: {},
  };
}

function view(halted: number | null, findings: Finding[]): ExploreView {
  return {
    archive: "mod.fantome",
    meta: null,
    thumbnail: null,
    halted,
    fault: null,
    findings,
    names: {},
    dating: null,
    wads: [],
    characters: [],
    units: [],
    kinds: [],
    sources: [],
  };
}

describe("kindRows", () => {
  it("keeps the order the report gives the kinds", () => {
    const rows = kindRows([
      { kind: "property-bin", files: 2, bytes: 500 },
      { kind: "texture", files: 9, bytes: 9000 },
      { kind: "text", files: 1, bytes: 10 },
    ]);
    expect(rows.map((row) => row.key)).toEqual(["property-bin", "texture", "text"]);
    expect(rows[1]).toEqual({ key: "texture", files: 9, bytes: 9000 });
  });
});

describe("sourceRows", () => {
  it("orders the facts as the library declares them, a fact it does not name last", () => {
    const rows = sourceRows([
      { fact: "own", files: 7, bytes: 70 },
      { fact: "later", files: 1, bytes: 1 },
      { fact: "history", files: 2, bytes: 20 },
      { fact: "install", files: 1, bytes: 10 },
    ]);
    expect(rows.map((row) => row.key)).toEqual(["install", "history", "own", "later"]);
  });
});

describe("entrySlotDiffers", () => {
  it("is true only for an entry that names another slot", () => {
    expect(entrySlotDiffers(unit(3, 0))).toBe(true);
    expect(entrySlotDiffers(unit(3, 3))).toBe(false);
    expect(entrySlotDiffers(unit(3, null))).toBe(false);
  });
});

describe("haltingFinding", () => {
  it("is the finding the view names as halting", () => {
    const halting = finding(4);
    expect(haltingFinding(view(4, [finding(1), halting]))).toBe(halting);
    expect(haltingFinding(view(null, [finding(1)]))).toBeNull();
  });
});

describe("endedEmpty", () => {
  it("is true for a run that ended before it knew anything of the mod", () => {
    expect(endedEmpty(view(0, [finding(0)]))).toBe(true);
    expect(endedEmpty({ ...view(null, []), fault: "the index did not answer" })).toBe(true);
  });

  it("is false for a run that ended its load, or ended early knowing some of the mod", () => {
    expect(endedEmpty(view(null, []))).toBe(false);
    const wad = {
      id: "%w0",
      name: "Test.wad.client",
      role: "champion",
      layout: "packed",
      files: 1,
      named: 1,
      packed_locale: null,
      restoration: false,
    };
    expect(endedEmpty({ ...view(0, [finding(0)]), wads: [wad] })).toBe(false);
  });
});
