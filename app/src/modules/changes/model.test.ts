import { describe, expect, it } from "vitest";

import type { ChangeView, FixupChanges, ObjectView } from "@/lib/bindings.gen";

import {
  countOutcomes,
  fieldCounts,
  groupBy,
  itemsOf,
  matchesText,
  movedParts,
  outcomeOfCode,
  splitEntry,
} from "./model";

const EMITTER = "VfxEmitterDefinitionData";

function change(over: Partial<ChangeView>): ChangeView {
  return {
    object: "%o1",
    entry: "ba578498",
    field: `${EMITTER}.birthScale1`,
    outcome: "changed",
    reached: true,
    path: "complexEmitterDefinitionData[4].birthScale1",
    before: { kind: "Embed", text: "ValueFloat {2 fields}" },
    after: { as: "moved", to: "complexEmitterDefinitionData[4].LegacySimple.birthScale" },
    ...over,
  };
}

function fixup(id: string, changes: ChangeView[]): FixupChanges {
  return { fixup: id, changed: 0, removed: 0, lost: 0, unconvertible: 0, changes };
}

const objects: ObjectView[] = [
  {
    id: "%o1",
    entry: "Characters/Urgot/Skins/Skin0/Particles/Urgot_Base_Q_Mis",
    class: "VfxSystemDefinitionData",
    bin: "data/characters/urgot/skins/skin0.bin",
  },
];

describe("changes", () => {
  it("reads the outcome off a fixup finding's code, and none off another's", () => {
    expect(outcomeOfCode("fixup.removed")).toBe("removed");
    expect(outcomeOfCode("fixup.lost")).toBe("lost");
    expect(outcomeOfCode("check.apply-failed")).toBeNull();
  });

  it("finds a change by any word of its object, field, path or values", () => {
    const [item] = itemsOf([fixup("vfx-legacy-simple", [change({})])], objects);
    expect(item.object?.class).toBe("VfxSystemDefinitionData");
    expect(matchesText(item, "urgot_base_q birthscale")).toBe(true);
    expect(matchesText(item, "LegacySimple")).toBe(true);
    expect(matchesText(item, "vfx-legacy")).toBe(true);
    expect(matchesText(item, "  ")).toBe(true);
    expect(matchesText(item, "birthScale flexshape")).toBe(false);
  });

  it("counts the outcomes and the fields, the most frequent field first", () => {
    const items = itemsOf(
      [
        fixup("a", [
          change({}),
          change({ field: `${EMITTER}.scale1` }),
          change({ field: `${EMITTER}.scale1`, outcome: "removed", after: { as: "removed" } }),
        ]),
      ],
      objects,
    );
    expect(countOutcomes(items)).toEqual({ changed: 2, removed: 1, lost: 0, unconvertible: 0 });
    expect(fieldCounts(items)).toEqual([
      { field: `${EMITTER}.scale1`, count: 2 },
      { field: `${EMITTER}.birthScale1`, count: 1 },
    ]);
  });

  it("groups in the order of each group's first item", () => {
    const groups = groupBy(["b1", "a1", "b2"], (text) => text[0]);
    expect(groups).toEqual([
      { key: "b", items: ["b1", "b2"] },
      { key: "a", items: ["a1"] },
    ]);
  });

  it("marks the segments a move added, and a renamed field", () => {
    expect(
      movedParts(
        "complexEmitterDefinitionData[3].scaleBirthScaleByBoundObjectSize",
        "complexEmitterDefinitionData[3].FlexShapeDefinition.scaleBirthScaleByBoundObjectSize",
      ),
    ).toEqual([
      { text: "complexEmitterDefinitionData[3]", added: false },
      { text: "FlexShapeDefinition", added: true },
      { text: "scaleBirthScaleByBoundObjectSize", added: false },
    ]);
    expect(movedParts("e[4].birthScale1", "e[4].LegacySimple.birthScale")).toEqual([
      { text: "e[4]", added: false },
      { text: "LegacySimple", added: true },
      { text: "birthScale", added: true },
    ]);
  });

  it("marks nothing for a struct taken out of its own field", () => {
    const parts = movedParts(
      "e[2].textureMult.textureMult.uvScaleMult",
      "e[2].textureMult.uvScaleMult",
    );
    expect(parts.every((part) => !part.added)).toBe(true);
    expect(parts.map((part) => part.text)).toEqual(["e[2]", "textureMult", "uvScaleMult"]);
  });

  it("splits an entry name at its last slash", () => {
    expect(splitEntry("Characters/Urgot/Skins/Skin0")).toEqual({
      folder: "Characters/Urgot/Skins/",
      name: "Skin0",
    });
    expect(splitEntry("ba578498")).toEqual({ folder: "", name: "ba578498" });
  });
});
