import { describe, expect, it } from "vitest";

import type { RunRow } from "@/lib/bindings.gen";

import { differingFirst, groupOthers, isDiffKey } from "./compare";

function run(id: number, sha256: string): RunRow {
  return {
    id,
    origin: "run",
    opened_from: null,
    made_at: "2026-10-11 10:00:00",
    kind: "port",
    kept: false,
    sha256,
    file_name: `mod-${id}.fantome`,
    name: null,
    author: null,
    version: null,
    package: null,
    reached: ["load"],
    archive_ingested: true,
  };
}

describe("groupOthers", () => {
  it("lists the runs of the same mod first and leaves the run itself out, in the list's order", () => {
    const runs = [run(5, "b"), run(4, "a"), run(3, "c"), run(2, "a"), run(1, "b")];
    const others = groupOthers(runs, runs[1]);
    expect(others.sameMod.map((row) => row.id)).toEqual([2]);
    expect(others.rest.map((row) => row.id)).toEqual([5, 3, 1]);
  });

  it("has no runs of the same mod when the run is its mod's only one", () => {
    const runs = [run(2, "a"), run(1, "b")];
    expect(groupOthers(runs, runs[0])).toEqual({ sameMod: [], rest: [runs[1]] });
  });
});

describe("differingFirst", () => {
  it("puts the rows that differ first and keeps the order within each group", () => {
    const rows = [
      { id: "a", differs: false },
      { id: "b", differs: true },
      { id: "c", differs: false },
      { id: "d", differs: true },
    ];
    expect(differingFirst(rows).map((row) => row.id)).toEqual(["b", "d", "a", "c"]);
  });
});

describe("isDiffKey", () => {
  it("knows the keys the store sends and no other", () => {
    expect(isDiffKey("fixups_off")).toBe(true);
    expect(isDiffKey("install_build")).toBe(true);
    expect(isDiffKey("unknown")).toBe(false);
  });
});
