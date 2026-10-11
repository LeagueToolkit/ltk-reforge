import { describe, expect, it } from "vitest";

import type { RunSettings } from "@/lib/bindings.gen";

import { advancedSettings, runSettings, useRepairStore } from "./repairStore";

const kept: RunSettings = {
  fixups: { off: ["retype"], on: [], rules: [] },
  checks_off: ["overlay"],
  keep_stale: true,
  keep_dds: false,
  block: ["error"],
  target: "base",
  anim_reuse: "allow",
  game_events_only: true,
  unit_targets: [{ character: "test", slot: 3, target: null }],
  kept_files: [{ wad: "test.wad.client", path: "assets/x.tex" }],
  keep_other_slots: true,
};

describe("a kept run run again", () => {
  it("gives the advanced view the settings its request had", () => {
    const advanced = advancedSettings("port", kept, "write");
    expect(runSettings(advanced)).toEqual(kept);
    expect(advanced.kind).toBe("port");
  });

  it("takes the archive, the kind and the settings, in the advanced view, through write", () => {
    useRepairStore
      .getState()
      .takeRun({ kind: "repair", archive: "C:\\mods\\a.fantome", settings: kept });
    const state = useRepairStore.getState();
    expect(state.archive).toBe("C:\\mods\\a.fantome");
    expect(state.mode).toBe("advanced");
    expect(state.advanced.kind).toBe("repair");
    expect(state.advanced.through).toBe("write");
    expect(state.last).toBeNull();
    expect(state.live).toBeNull();
  });
});
