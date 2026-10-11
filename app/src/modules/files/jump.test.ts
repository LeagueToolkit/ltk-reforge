import { describe, expect, it } from "vitest";

import type { FileRow, StageFiles } from "@/lib/bindings.gen";

import { placeKey, placeRow } from "./jump";

function row(path: string | null, mark: FileRow["mark"] = "same"): FileRow {
  return {
    hash: "0123456789abcdef",
    path,
    content: "c".repeat(64),
    size: 1,
    kind: "property-bin",
    mark,
    source: null,
  };
}

const files: StageFiles = {
  stage: "build",
  wads: [
    { name: "Test.wad.client", files: [row("data/characters/test/skins/skin0.bin")] },
    { name: "hud/Other.wad.client", files: [row("data/other.bin")] },
    { name: "Gone.wad.client", files: [row("data/gone.bin", "removed")] },
  ],
};

describe("placeRow", () => {
  it("finds a place's bin by its WAD's name and its path, in any case", () => {
    const found = placeRow(files, "Test.wad.client", "DATA/Characters/Test/Skins/Skin0.bin");
    expect(found?.path).toBe("data/characters/test/skins/skin0.bin");
  });

  it("finds a WAD of another layer under the layer's name", () => {
    expect(placeRow(files, "Other.wad.client", "data/other.bin")?.path).toBe("data/other.bin");
  });

  it("finds no file for a game bin a patch changes, or one the package removed", () => {
    expect(placeRow(files, "Test.wad.client", "data/characters/test/test.bin")).toBeNull();
    expect(placeRow(files, "Elsewhere.wad.client", "data/other.bin")).toBeNull();
    expect(placeRow(files, "Gone.wad.client", "data/gone.bin")).toBeNull();
  });
});

describe("placeKey", () => {
  it("keys a place's value as the tree keys its row: the entry with `0x`, then the path", () => {
    expect(placeKey({ entry: "2a1f3c7d", at: "0000000b[3].0000000c" })).toBe(
      "0x2a1f3c7d:0000000b[3].0000000c",
    );
  });

  it("keys the object itself for a place with no path", () => {
    expect(placeKey({ entry: "2a1f3c7d", at: "" })).toBe("0x2a1f3c7d:");
  });
});
