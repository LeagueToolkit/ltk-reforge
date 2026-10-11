import { describe, expect, it } from "vitest";

import { nearestScrollTop } from "./scroll";

const box = { top: 100, height: 200 };

describe("nearestScrollTop", () => {
  it("leaves a row already in view where it is", () => {
    expect(nearestScrollTop(box, 40, { top: 150, height: 20 })).toBe(40);
  });

  it("brings a row above the box to its top edge", () => {
    expect(nearestScrollTop(box, 40, { top: 90, height: 20 })).toBe(30);
  });

  it("brings a row below the box to its bottom edge", () => {
    expect(nearestScrollTop(box, 40, { top: 310, height: 20 })).toBe(70);
  });

  it("shows the top of a row taller than the box", () => {
    expect(nearestScrollTop(box, 40, { top: 150, height: 400 })).toBe(90);
  });
});
