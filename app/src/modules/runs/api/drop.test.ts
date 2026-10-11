import { describe, expect, it } from "vitest";

import { droppedOpen } from "./drop";

describe("droppedOpen", () => {
  it("opens a project file as a project and a package as a package, by extension in any case", () => {
    expect(droppedOpen(["C:\\runs\\mod.REFORGE-PROJECT"])).toEqual({
      kind: "project",
      path: "C:\\runs\\mod.REFORGE-PROJECT",
    });
    expect(droppedOpen(["/home/a/Mod_1.0.0.modpkg"])).toEqual({
      kind: "package",
      path: "/home/a/Mod_1.0.0.modpkg",
    });
  });

  it("takes the first path that opens, past the ones that do not", () => {
    expect(
      droppedOpen(["C:\\mods\\a.fantome", "C:\\out\\b.modpkg", "C:\\c.reforge-project"]),
    ).toEqual({ kind: "package", path: "C:\\out\\b.modpkg" });
  });

  it("opens nothing for a mod archive, a report or no path", () => {
    expect(droppedOpen(["C:\\mods\\a.fantome", "C:\\out\\b.reforge-report.json"])).toBeNull();
    expect(droppedOpen(["C:\\out\\modpkg", "C:\\out\\reforge-project"])).toBeNull();
    expect(droppedOpen([])).toBeNull();
  });
});
