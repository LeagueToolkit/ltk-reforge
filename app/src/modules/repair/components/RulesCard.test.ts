import { describe, expect, it } from "vitest";

import { parseRows, ruleOf } from "./RulesCard";

describe("parseRows", () => {
  it("reads a row per line, a field that keeps its name written once", () => {
    expect(parseRows("a -> b\n\n  c  \n")).toEqual([
      { from: "a", to: "b" },
      { from: "c", to: "c" },
    ]);
  });
});

describe("ruleOf", () => {
  const draft = {
    id: " my-rule ",
    title: "",
    class: "VfxEmitterDefinitionData",
    op: "remove" as const,
    fields: "toolField\nother",
    childField: "",
    childClass: "",
  };

  it("makes a removal of the fields named, in the phase that undoes a tool's damage", () => {
    const rule = ruleOf(draft);
    expect(rule.id).toBe("my-rule");
    expect(rule.title).toBeNull();
    expect(rule.phase).toEqual({ name: "undo" });
    expect(rule.op).toEqual({ op: "remove", fields: ["toolField", "other"] });
  });

  it("makes a move into the child named", () => {
    const rule = ruleOf({
      ...draft,
      op: "move",
      childField: "f",
      childClass: "C",
      fields: "a -> b",
    });
    expect(rule.op).toEqual({
      op: "move",
      into: { field: "f", class: "C" },
      fields: [{ from: "a", to: "b" }],
    });
  });
});
