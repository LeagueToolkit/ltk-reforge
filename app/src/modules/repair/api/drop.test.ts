import { describe, expect, it } from "vitest";

import { droppedArchives } from "./drop";

describe("droppedArchives", () => {
  it("takes every archive dropped, by its extension in any case, in the order dropped", () => {
    expect(
      droppedArchives(["C:\\mods\\readme.txt", "C:\\mods\\b.FANTOME", "C:\\mods\\a.zip"]),
    ).toEqual(["C:\\mods\\b.FANTOME", "C:\\mods\\a.zip"]);
    expect(droppedArchives(["/home/a/mod.zip", "/home/a/other.fantome"])).toEqual([
      "/home/a/mod.zip",
      "/home/a/other.fantome",
    ]);
  });

  it("takes nothing when no path is an archive", () => {
    expect(droppedArchives(["C:\\mods\\folder", "C:\\mods\\a.png", "C:\\mods\\a.zip.txt"])).toEqual(
      [],
    );
    expect(droppedArchives([])).toEqual([]);
  });
});
