import { describe, expect, it } from "vitest";

import type { FileRow, StageFiles } from "@/lib/bindings.gen";

import { ENTRY_FENCE, fencedObject } from "./fence";
import { placeRow } from "./jump";

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

describe("fencedObject", () => {
  const fence = `${ENTRY_FENCE}\n`;

  it("splits the object asked for from the objects after it", () => {
    const text = `${fence}a = Material {}\n${fence}b = Material {}\n`;
    expect(fencedObject(text)).toEqual({ asked: "a = Material {}\n", rest: "b = Material {}\n" });
  });

  it("leaves text with no fences as it is", () => {
    expect(fencedObject("a = Material {}\n")).toEqual({ asked: null, rest: "a = Material {}\n" });
    expect(fencedObject(`${fence}a = Material {}\n`)).toEqual({
      asked: null,
      rest: `${fence}a = Material {}\n`,
    });
  });
});
