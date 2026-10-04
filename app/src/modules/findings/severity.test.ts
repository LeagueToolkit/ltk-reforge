import { describe, expect, it } from "vitest";

import type { Finding, Severity } from "@/lib/bindings.gen";

import { countBySeverity, worstFirst } from "./severity";

function finding(id: number, severity: Severity): Finding {
  return {
    id,
    code: "test.code",
    severity,
    channel: "output",
    stage: "fix",
    subject: { kind: "mod", id: "", name: "" },
    by: null,
    record: null,
    message: "",
  };
}

describe("worstFirst", () => {
  it("puts errors before warnings and keeps the order raised within each", () => {
    const findings = [
      finding(1, "note"),
      finding(2, "error"),
      finding(3, "warning"),
      finding(4, "error"),
    ];
    expect(worstFirst(findings).map((f) => f.id)).toEqual([2, 4, 3, 1]);
  });
});

describe("countBySeverity", () => {
  it("counts each severity", () => {
    const counts = countBySeverity([finding(1, "note"), finding(2, "note"), finding(3, "fatal")]);
    expect(counts).toEqual({ fatal: 1, error: 0, warning: 0, note: 2 });
  });
});
