import { describe, expect, it } from "vitest";

import type { ManagerMod } from "@/lib/bindings.gen";

import { matchesFilter } from "./components/ManagerPage";
import { isLegacyChampionMod, noteOf } from "./legacy";

function mod(fields: Partial<ManagerMod>): ManagerMod {
  return {
    slug: "a",
    name: "A",
    version: null,
    format: "fantome",
    archive: "C:/mods/a.fantome",
    extended: false,
    installed_at: "",
    champions: ["Vex"],
    maps: [],
    tags: ["champion-skin"],
    ...fields,
  };
}

describe("isLegacyChampionMod", () => {
  it("takes a plain fantome of a champion with its archive", () => {
    expect(isLegacyChampionMod(mod({}))).toBe(true);
    expect(isLegacyChampionMod(mod({ tags: [], champions: ["Vex"] }))).toBe(true);
  });

  it("leaves the rest, each for its reason", () => {
    expect(noteOf(mod({ archive: null }))).toBe("no-archive");
    expect(noteOf(mod({ format: "modpkg" }))).toBe("modpkg");
    expect(noteOf(mod({ extended: true }))).toBe("extended");
    expect(noteOf(mod({ tags: [], champions: [], maps: ["Map11"] }))).toBe("not-champion");
    expect(noteOf(mod({}))).toBe(null);
  });
});

describe("matchesFilter", () => {
  it("matches the name, slug, version or a champion, whatever the case", () => {
    const one = mod({ name: "Mimikyu Vex", slug: "mimikyu-vex", version: "1.0.0" });
    expect(matchesFilter(one, "")).toBe(true);
    expect(matchesFilter(one, "  VEX ")).toBe(true);
    expect(matchesFilter(one, "mimikyu-")).toBe(true);
    expect(matchesFilter(one, "1.0")).toBe(true);
    expect(matchesFilter(one, "janna")).toBe(false);
  });
});
