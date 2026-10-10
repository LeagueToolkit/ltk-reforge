import { describe, expect, it } from "vitest";

import { subjectName } from "./components/DataTree";

describe("subjectName", () => {
  it("spells a subject raised before its stage named the tables", () => {
    const subject = { id: "%u0", name: "%u0" };
    expect(subjectName(subject, {})).toBe("%u0");
    expect(subjectName(subject, { "%u0": "Urgot skin5" })).toBe("Urgot skin5");
  });

  it("keeps the name of a subject no table names", () => {
    expect(subjectName({ id: "run", name: "run" }, { "%w0": "Urgot.wad.client" })).toBe("run");
  });
});
