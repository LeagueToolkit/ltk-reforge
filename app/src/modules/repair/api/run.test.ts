import { describe, expect, it } from "vitest";

import { folderOf, liveStage } from "./run";

describe("folderOf", () => {
  it("cuts a Windows path at its last separator", () => {
    expect(folderOf("C:\\Mods\\Ahri\\ahri.fantome")).toBe("C:\\Mods\\Ahri");
  });

  it("cuts a path of forward slashes", () => {
    expect(folderOf("/home/a/mods/x.zip")).toBe("/home/a/mods");
  });
});

describe("liveStage", () => {
  it("takes a stage from a kept run as resumed, with no time", () => {
    expect(liveStage({ event: "resumed" })).toEqual({ state: "resumed" });
    expect(liveStage({ event: "ended", millis: 4 })).toEqual({ state: "ended", millis: 4 });
    expect(liveStage({ event: "started" })).toEqual({ state: "running" });
  });
});
