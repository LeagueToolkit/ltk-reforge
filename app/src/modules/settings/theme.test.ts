import { describe, expect, it } from "vitest";

import { drawnTheme } from "./theme";

describe("drawnTheme", () => {
  it("follows the system while the setting is system", () => {
    expect(drawnTheme("system", true)).toBe("dark");
    expect(drawnTheme("system", false)).toBe("light");
  });

  it("keeps a chosen theme whatever the system prefers", () => {
    expect(drawnTheme("light", true)).toBe("light");
    expect(drawnTheme("dark", false)).toBe("dark");
  });
});
