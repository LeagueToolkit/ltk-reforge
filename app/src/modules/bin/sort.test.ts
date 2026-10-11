/** Copied from LTK Manager's `src/modules/workshop/bin/tree/utils/__tests__/binRows.sort.test.ts`. */
import { describe, expect, it } from "vitest";

import type { BinRow } from "@/lib/bindings.gen";

import { sortedRoots } from "./rows";

function object(name: string, className: string | null, unnamed = false): BinRow {
  return {
    entry: name,
    path: "",
    label: "",
    node: "object",
    name,
    unnamed,
    kind: null,
    value: { type: "struct", classHash: "0x0000000a", class: className, len: 1 },
  };
}

function target(name: string): BinRow {
  return { ...object(name, null), node: "target", path: "#", value: { type: "records", len: 1 } };
}

function names(rows: readonly BinRow[]): string[] {
  return rows.map((row) => row.name);
}

describe("sortedRoots", () => {
  it("lists a file's objects by class, and by name within one", () => {
    const rows = [
      object("Maps/Chunks/Plants", "MapPlaceableContainer"),
      object("Materials/Water", "StaticMaterialDef"),
      object("Maps/Base_SRX", "MapContainer"),
      object("Materials/Grass", "StaticMaterialDef"),
    ];

    expect(names(sortedRoots(rows))).toEqual([
      "Maps/Base_SRX",
      "Maps/Chunks/Plants",
      "Materials/Grass",
      "Materials/Water",
    ]);
  });

  it("reads digits by their value and takes no notice of case", () => {
    const rows = [
      object("Skin10", "SkinCharacterDataProperties"),
      object("skin2", "SkinCharacterDataProperties"),
      object("Skin1", "SkinCharacterDataProperties"),
    ];

    expect(names(sortedRoots(rows))).toEqual(["Skin1", "skin2", "Skin10"]);
  });

  it("lists what nothing names after what is named, a class and an object alike", () => {
    const rows = [
      object("0x00000002", "StaticMaterialDef", true),
      object("0x00000001", null, true),
      object("Materials/Grass", "StaticMaterialDef"),
    ];

    expect(names(sortedRoots(rows))).toEqual(["Materials/Grass", "0x00000002", "0x00000001"]);
  });

  it("keeps a patch's targets after the objects, and leaves the rows it was handed alone", () => {
    const rows = [target("Characters/Zed"), object("Materials/Grass", "StaticMaterialDef")];

    expect(names(sortedRoots(rows))).toEqual(["Materials/Grass", "Characters/Zed"]);
    expect(names(rows)).toEqual(["Characters/Zed", "Materials/Grass"]);
  });
});
