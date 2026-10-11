import { describe, expect, it } from "vitest";

import { countsLabel } from "./RunSummary";

describe("countsLabel", () => {
  it("names a count of one in the singular and the fatal findings among the errors", () => {
    expect(countsLabel({ fatal: 0, error: 1, warning: 0, note: 36 })).toBe(
      "1 error, 0 warnings, 36 notes",
    );
    expect(countsLabel({ fatal: 1, error: 1, warning: 1, note: 1 })).toBe(
      "2 errors, 1 warning, 1 note",
    );
  });
});
