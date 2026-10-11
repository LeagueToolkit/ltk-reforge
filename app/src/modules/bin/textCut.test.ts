/** Copied from LTK Manager's `src/modules/workshop/bin/shared/utils/__tests__/textCut.test.ts`. */
import { describe, expect, it } from "vitest";

import { charsIn, cutText } from "./textCut";

describe("cutText", () => {
  it("leaves a name that fits alone", () => {
    expect(cutText("BaseEnergyBlend", 20)).toBe("BaseEnergyBlend");
    expect(cutText("BaseEnergyBlend", null)).toBe("BaseEnergyBlend");
  });

  it("cuts the middle of a name and keeps both ends", () => {
    expect(cutText("BaseEnergyFlares", 11)).toBe("BaseE…lares");
  });

  it("draws the ellipsis alone where one character fits", () => {
    expect(cutText("BaseEnergyBlend", 1)).toBe("…");
    expect(cutText("BaseEnergyBlend", 0)).toBe("");
  });
});

describe("charsIn", () => {
  it("counts the characters a box holds", () => {
    expect(charsIn(100, 7)).toBe(14);
  });

  it("answers null before anything is measured", () => {
    expect(charsIn(0, 7)).toBeNull();
    expect(charsIn(100, 0)).toBeNull();
  });
});
