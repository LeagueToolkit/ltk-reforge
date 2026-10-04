import { describe, expect, it } from "vitest";

import { folderOf } from "./run";

describe("folderOf", () => {
  it("cuts a Windows path at its last separator", () => {
    expect(folderOf("C:\\Mods\\Ahri\\ahri.fantome")).toBe("C:\\Mods\\Ahri");
  });

  it("cuts a path of forward slashes", () => {
    expect(folderOf("/home/a/mods/x.zip")).toBe("/home/a/mods");
  });
});
