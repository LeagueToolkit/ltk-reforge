import { describe, expect, it } from "vitest";

import { droppedArchive } from "./drop";

describe("droppedArchive", () => {
  it("takes the first archive dropped, by its extension in any case", () => {
    expect(droppedArchive(["C:\\mods\\readme.txt", "C:\\mods\\Urgot.FANTOME"])).toBe(
      "C:\\mods\\Urgot.FANTOME",
    );
    expect(droppedArchive(["/home/a/mod.zip", "/home/a/other.fantome"])).toBe("/home/a/mod.zip");
  });

  it("takes nothing when no path is an archive", () => {
    expect(droppedArchive(["C:\\mods\\Urgot", "C:\\mods\\a.png"])).toBeNull();
    expect(droppedArchive([])).toBeNull();
  });
});
