/** Copied from LTK Manager's `src/modules/workshop/bin/values/utils/__tests__/kindTag.test.ts`. */
import { describe, expect, it } from "vitest";

import { rowTag, shapeTag } from "./kindTag";

describe("shapeTag", () => {
  it("writes a leaf as its kind alone", () => {
    expect(shapeTag({ kind: "string", key: null, value: null })).toBe("string");
    expect(shapeTag({ kind: "rgba", key: null, value: null })).toBe("rgba");
  });

  it("writes what a list or an option holds in brackets", () => {
    expect(shapeTag({ kind: "list", key: null, value: "embed" })).toBe("list[embed]");
    expect(shapeTag({ kind: "list2", key: null, value: "file" })).toBe("list2[file]");
    expect(shapeTag({ kind: "option", key: null, value: "f32" })).toBe("option[f32]");
  });

  it("writes a map's key and value, comma-joined", () => {
    expect(shapeTag({ kind: "map", key: "hash", value: "string" })).toBe("map[hash,string]");
  });
});

describe("rowTag", () => {
  it("composes a container's tag from the kind of its items", () => {
    expect(rowTag({ kind: "list", value: { type: "container", len: 8, itemKind: "embed" } })).toBe(
      "list[embed]",
    );
  });

  it("composes an optional's tag whether or not it holds a value", () => {
    expect(
      rowTag({ kind: "option", value: { type: "optional", present: true, itemKind: "f32" } }),
    ).toBe("option[f32]");
    expect(
      rowTag({ kind: "option", value: { type: "optional", present: false, itemKind: "string" } }),
    ).toBe("option[string]");
  });

  it("composes a map's tag from both of its kinds", () => {
    expect(
      rowTag({ kind: "map", value: { type: "map", len: 3, keyKind: "hash", valueKind: "string" } }),
    ).toBe("map[hash,string]");
  });

  it("tags a leaf and an embed with the kind alone", () => {
    expect(rowTag({ kind: "string", value: { type: "string", value: "Justicar Aatrox" } })).toBe(
      "string",
    );
    expect(
      rowTag({
        kind: "embed",
        value: { type: "struct", classHash: "0x9b67e9f6", class: "SkinMeshDataProperties", len: 2 },
      }),
    ).toBe("embed");
  });

  it("gives an object row no tag", () => {
    expect(
      rowTag({
        kind: null,
        value: { type: "struct", classHash: "0x9b67e9f6", class: "CharacterRecord", len: 2 },
      }),
    ).toBeNull();
  });
});
